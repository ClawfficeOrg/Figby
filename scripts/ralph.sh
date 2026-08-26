#!/bin/sh
# ralph — autonomous task loop for the Figby repository.
#
# Usage:
#   ./scripts/ralph.sh                          # multi-phase: loop through all open phases from main
#   ./scripts/ralph.sh 1.1.1                    # single task (infers release branch)
#   ./scripts/ralph.sh --minutes=30             # loop for up to 30 minutes
#   ./scripts/ralph.sh --hours=2                # loop for up to 2 hours
#   ./scripts/ralph.sh --hours=1 --minutes=30   # loop for up to 1 h 30 m
#   ./scripts/ralph.sh --minutes=45 1.1.4       # single task, still time-bounded
#   ./scripts/ralph.sh --until=1.1.3            # run all open tasks up to and including 1.1.3, then stop
#   ./scripts/ralph.sh --dry-run                # preview the next action and exit
#   ./scripts/ralph.sh --log=/tmp/my-run.log    # log everything to a file
#   ./scripts/ralph.sh --quiet                  # suppress agent stderr (default: verbose)
#
# Ralph can be started from:
#   main           — picks up the next open phase and loops through all phases.
#   release/X.Y    — resumes that phase, then continues through subsequent
#                    phases automatically after each one merges to main.
#
# In both cases the loop only stops when no open tasks remain, the time
# limit is reached, or a graceful stop is requested.
#
# Stopping ralph gracefully (it will finish the current task first):
#   touch scripts/STOP.md               # drop a sentinel file in the repo
#   kill -TERM $(cat /tmp/ralph.pid)    # or send SIGTERM to the process
#   Ctrl-C                              # or SIGINT from the terminal
#
# Requires: opencode CLI, cargo, git, a clean working tree.
# The human has given blanket commit+push permission while this runs.

set -e

REPO_ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
SKILL="$REPO_ROOT/skills/ralph.md"
LOG="$REPO_ROOT/docs/ralph-log.md"
MANIFEST="--manifest-path figby-rs/Cargo.toml"
# Resolve actual default branch (master vs main)
DEFAULT_BRANCH="$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's|refs/remotes/origin/||' || echo main)"

# Agent Configuration
# Each agent is a PROVIDER/MODEL pair (e.g. "opencode-go/deepseek-v4-flash").
# Provider determines which CLI binary is invoked:
#   opencode-go    → opencode
#   kilocode       → kilo
# Model is passed directly as --model to the CLI.
# All agents can be overridden at runtime via environment variables.

TASK_PLANNING_AGENT="${TASK_PLANNING_AGENT:-opencode-go/deepseek-v4-flash}"
BASIC_DEV_AGENT="${BASIC_DEV_AGENT:-opencode-go/deepseek-v4-flash}"
MID_DEV_AGENT="${MID_DEV_AGENT:-opencode-go/deepseek-v4-flash}"
PRO_DEV_AGENT="${PRO_DEV_AGENT:-opencode-go/deepseek-v4-flash}"
TASK_REVIEW_AGENT="${TASK_REVIEW_AGENT:-opencode-go/deepseek-v4-flash}"
RELEASE_REVIEW_AGENT="${RELEASE_REVIEW_AGENT:-opencode-go/deepseek-v4-flash}"
MAJOR_RELEASE_REVIEW_AGENT="${MAJOR_RELEASE_REVIEW_AGENT:-opencode-go/deepseek-v4-flash}"
ARCHITECT_AGENT="${ARCHITECT_AGENT:-opencode-go/deepseek-v4-flash}"

BASE_BRANCH="$(git -C "$REPO_ROOT" rev-parse --abbrev-ref HEAD)"
# Normalize master → main for consistent checks
[ "$BASE_BRANCH" = "master" ] && BASE_BRANCH="main"

# — colours —
BOLD=$(tput bold 2>/dev/null || true)
CYAN=$(tput setaf 6 2>/dev/null || true)
GREEN=$(tput setaf 2 2>/dev/null || true)
YELLOW=$(tput setaf 3 2>/dev/null || true)
RED=$(tput setaf 1 2>/dev/null || true)
RESET=$(tput sgr0 2>/dev/null || true)

log() { printf '%s\n' "${CYAN}[ralph]${RESET} $*"; }
good() { printf '%s\n' "${GREEN}[ralph]${RESET} $*"; }
warn() { printf '%s\n' "${YELLOW}[ralph]${RESET} $*"; }
die() {
  printf '%s\n' "${RED}[ralph]${RESET} $*" >&2
  exit 1
}

# — argument parsing —
SINGLE_TASK=""
UNTIL_TASK=""
DURATION_SECS=0
DRY_RUN=0
QUIET=0
RALPH_LOG_FILE="${RALPH_LOG_FILE:-}"

for _arg in "$@"; do
  case "$_arg" in
  --minutes=*)
    _mins="${_arg#--minutes=}"
    case "$_mins" in '' | *[!0-9]*) die "--minutes requires a positive integer" ;; esac
    DURATION_SECS=$((DURATION_SECS + _mins * 60))
    ;;
  --hours=*)
    _hrs="${_arg#--hours=}"
    case "$_hrs" in '' | *[!0-9]*) die "--hours requires a positive integer" ;; esac
    DURATION_SECS=$((DURATION_SECS + _hrs * 3600))
    ;;
  --until=*)
    UNTIL_TASK="${_arg#--until=}"
    [ -n "$UNTIL_TASK" ] || die "--until requires a task id (e.g. --until=1.1.3)"
    ;;
  --dry-run)
    DRY_RUN=1
    ;;
  --quiet | -q)
    QUIET=1
    ;;
  --log=*)
    _log="${_arg#--log=}"
    [ -n "$_log" ] || die "--log requires a file path"
    RALPH_LOG_FILE="$_log"
    ;;
  -*)
    die "Unknown flag: $_arg  (supported: --minutes=N  --hours=N  --until=TASK_ID  --dry-run  --quiet/-q  --log=FILE)"
    ;;
  *)
    [ -z "$SINGLE_TASK" ] || die "Too many positional arguments — only one task id is allowed"
    SINGLE_TASK="$_arg"
    ;;
  esac
done

START_TIME="$(date +%s)"
if [ "$DURATION_SECS" -gt 0 ]; then
  DEADLINE=$((START_TIME + DURATION_SECS))
else
  DEADLINE=0
fi

# — session logging —
if [ -n "$RALPH_LOG_FILE" ] && [ -z "${RALPH_LOGGING_ACTIVE:-}" ]; then
  export RALPH_LOGGING_ACTIVE=1
  printf '%s\n' "${CYAN}[ralph]${RESET} Logging session to ${RALPH_LOG_FILE}"
  { "$0" "$@"; } 2>&1 | tee -a "$RALPH_LOG_FILE"
  exit $?
fi

# — sanity checks —
[ -f "$SKILL" ] || die "skill file missing: $SKILL"
command -v opencode >/dev/null 2>&1 || die "opencode CLI not found in PATH — install from https://github.com/opencode-ai/opencode"

case "$BASE_BRANCH" in
main|master)
  MINOR_VERSION=""
  ;;
release/*)
  MINOR_VERSION="${BASE_BRANCH#release/}"
  case "$MINOR_VERSION" in
  *[!0-9.]* | "") die "release branch '${BASE_BRANCH}' has an invalid minor version '${MINOR_VERSION}'" ;;
  esac
  ;;
*)
  die "ralph must be started from 'main' (or 'master') or a 'release/X.Y' branch, not '${BASE_BRANCH}'.\n" \
    "  From main/master (multi-phase — recommended for unattended runs):\n" \
    "    git checkout main/master && ./scripts/ralph.sh\n" \
    "  From a specific release branch (single-phase):\n" \
    "    git checkout release/1.1 && ./scripts/ralph.sh"
  ;;
esac

if [ -n "$SINGLE_TASK" ] && [ "$BASE_BRANCH" != "main" ]; then
  TASK_MINOR="$(printf '%s' "$SINGLE_TASK" | sed 's/\.[0-9]*$//')"
  [ "$TASK_MINOR" = "$MINOR_VERSION" ] ||
    die "Task ${SINGLE_TASK} (minor: ${TASK_MINOR}) does not belong to ${BASE_BRANCH} (minor: ${MINOR_VERSION})."
fi

if [ "$DRY_RUN" -eq 1 ]; then
  log "Dry run: no changes will be made."
  if [ -n "$SINGLE_TASK" ]; then
    if [ "$BASE_BRANCH" = "main" ]; then
      _task_minor="$(printf '%s' "$SINGLE_TASK" | sed 's/\.[0-9]*$//')"
      log "Would switch to release/${_task_minor} and run the next open task there."
    else
      log "Would run task ${SINGLE_TASK} on ${BASE_BRANCH}."
    fi
    exit 0
  fi
  if [ "$BASE_BRANCH" = "main" ]; then
    log "Would switch to the next open release branch and start its first task."
    exit 0
  fi
  if [ "${MINOR_VERSION}" != "${MINOR_VERSION%.0}" ]; then
    log "Would create rc/${MINOR_VERSION}.0.0-rc.1 and stop for human sign-off."
  else
    log "Would run the next task or phase review for ${MINOR_VERSION}."
  fi
  exit 0
fi

cd "$REPO_ROOT"

if ! git diff --quiet || ! git diff --cached --quiet || [ -n "$(git ls-files --others --exclude-standard 2>/dev/null)" ]; then
  _dirty_files="$( (git diff --name-only; git diff --cached --name-only; git ls-files --others --exclude-standard) 2>/dev/null | sort -u | grep -v '^$' || true)"
  _non_log="$(printf '%s' "$_dirty_files" | grep -v '^docs/ralph-log.md$' || true)"
  if [ -z "$_non_log" ]; then
    log "Dirty tree from ralph-log.md — checking for unmarked completed tasks..."
    git add docs/ralph-log.md
    _last_done="$(grep -oP 'DONE: \K[\d]+\.[\d]+\.[\d]+' docs/ralph-log.md | tail -1)"
    if [ -n "$_last_done" ]; then
      _todo_file=""
      for _f in "$REPO_ROOT/docs"/todo-v*.md; do
        if grep -q "\`${_last_done}\`" "$_f" 2>/dev/null; then
          _todo_file="$_f"
          break
        fi
      done
      if [ -n "$_todo_file" ] && grep -q "\- \[ \] \`${_last_done}\`" "$_todo_file" 2>/dev/null; then
        log "Marking ${_last_done} as done (log says merged, checkbox stale)."
        sed -i "s/^- \[ \] \`${_last_done}\`/- [x] \`${_last_done}\`/" "$_todo_file"
        git add "$_todo_file"
      fi
    fi
    git commit -m "docs: ralph-log continuation checkpoint"
    git push origin "$BASE_BRANCH" 2>/dev/null || true
    good "Log checkpoint committed. Continuing."
  else
    die "working tree is dirty — commit or stash changes before running ralph"
  fi
fi

# — F-20 R6: install the real pre-commit gate (repo-local) —
# gates live in .githooks/pre-commit; ralph.sh ALSO runs them explicitly
# before any commit, so a missing/inactive hook never means "no gate".
if ! git config --get core.hooksPath >/dev/null 2>&1; then
  git config core.hooksPath .githooks
  log "Set core.hooksPath=.githooks (pre-commit gate installed)."
fi

# — PID file + signal / sentinel stop mechanism —
RALPH_PID_FILE="/tmp/ralph.pid"
STOP_SENTINEL="$REPO_ROOT/scripts/STOP.md"
STOP_REQUESTED=0

printf '%d\n' $$ >"$RALPH_PID_FILE"

# caveman mode for opencode agents
CAVEMAN_FLAG_PATH="${XDG_CONFIG_HOME:-$HOME/.config}/opencode/.caveman-active"
mkdir -p "$(dirname "$CAVEMAN_FLAG_PATH")"
printf '%s\n' 'full' >"$CAVEMAN_FLAG_PATH"

trap 'rm -f "$RALPH_PID_FILE"' EXIT
trap 'STOP_REQUESTED=1; warn "Stop signal received — will exit cleanly after the current task."' INT TERM

# — announce —
log "PID $$ written to $RALPH_PID_FILE"
if [ -n "$MINOR_VERSION" ]; then
  log "Mode: single-phase  branch: ${BASE_BRANCH}  phase: ${MINOR_VERSION}"
else
  log "Mode: multi-phase  starting from ${BASE_BRANCH} (default: ${DEFAULT_BRANCH}) — will create release branches as needed"
fi
log "To stop gracefully:  kill -TERM \$(cat $RALPH_PID_FILE)  or  touch $STOP_SENTINEL"
log "Caveman mode: full (opencode agents will use terse response style)"

if [ "$DURATION_SECS" -gt 0 ]; then
  _human="$((DURATION_SECS / 3600))h $(((DURATION_SECS % 3600) / 60))m"
  _deadline_str="$(date -r "$DEADLINE" '+%H:%M:%S' 2>/dev/null ||
    date -d "@$DEADLINE" '+%H:%M:%S' 2>/dev/null ||
    printf 'epoch %d' "$DEADLINE")"
  log "Time limit: ${_human} (deadline ${_deadline_str})"
fi

# — helpers —

all_todo_lines() {
  for _f in "$REPO_ROOT/docs"/todo-v*.md; do
    [ -f "$_f" ] && cat "$_f" || true
  done
}

task_for_minor() {
  _minor="$1"
  all_todo_lines | grep -m1 "^\- \[ \] \`${_minor}\.[0-9]\+\`" |
    sed "s/^- \[ \] \`\([^\`]*\)\`.*/\1/" || true
}

# — agent helpers —

agent_provider() {
  printf '%s' "$1" | sed 's|/.*||'
}

agent_model() {
  printf '%s' "$1" | sed 's|^[^/]*/||'
}

agent_cli() {
  case "$(agent_provider "$1")" in
  opencode-go) printf 'opencode' ;;
  kilocode) printf 'kilo' ;;
  github-copilot) printf 'copilot' ;;
  claude-code) printf 'claude' ;;
  *) printf 'opencode' ;;
  esac
}

AGENT_TIMEOUT="${RALPH_AGENT_TIMEOUT:-300}"

# — F-20 sandbox, gate & staging helpers (see docs/f-20-ralph-sandbox-policy.md) —

# Emit the names of credential-shaped environment variables to UNSET before
# invoking an agent. Provider model keys (the minimum needed to run the model
# at all) are retained; their blast radius is model-credit spend rather than
# repo/service access, and the tool allowlist denies git + network, so they
# cannot be exfiltrated to an attacker endpoint. Everything else that looks
# like a secret is removed so a compromised agent cannot read it.
scrub_agent_env() {
  _keep="${RALPH_KEEP_PROVIDER_KEYS:-ANTHROPIC_API_KEY OPENAI_API_KEY OPENROUTER_API_KEY GEMINI_API_KEY DEEPSEEK_API_KEY OPENAI_BASE_URL}"
  _scrub="GITHUB_TOKEN GH_TOKEN GITLAB_TOKEN BITBUCKET_TOKEN CIRCLE_TOKEN TRAVIS_TOKEN GIT_SSH_COMMAND GIT_ASKPASS AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_SESSION_TOKEN AWS_PROFILE AZURE_CLIENT_SECRET AZURE_TENANT_ID GOOGLE_API_KEY GCP_SA_KEY DO_API_TOKEN CF_API_TOKEN OPENCODE_SERVER_PASSWORD NETLIFY_AUTH_TOKEN VERCEL_TOKEN NPM_TOKEN CARGO_REGISTRY_TOKEN"
  _ev=""
  for _ev in $(env | cut -d= -f1); do
    case "$_ev" in
      *TOKEN*|*KEY*|*SECRET*|*PASSWORD*|*CREDENTIAL*)
        case " $_keep " in
          *" $_ev "*) ;;
          *) _scrub="$_scrub $_ev" ;;
        esac
        ;;
    esac
  done
  for _ev in $_scrub; do
    printf '%s\n' "$_ev"
  done
}

# write_sandbox_config <file> — write an opencode permissions profile that
# allows file read/write/edit and the four cargo verify commands while
# denying git, package managers, network fetch, sudo, and credential-file
# reads (F-20 R1/R2). `external_directory` deny keeps the built-in
# read/edit/glob tools inside the working tree.
write_sandbox_config() {
  _out="$1"
  cat > "$_out" <<'EOF'
{
  "$schema": "https://opencode.ai/config.json",
  "permission": {
    "read": "allow",
    "glob": "allow",
    "grep": "allow",
    "list": "allow",
    "write": "allow",
    "edit": "allow",
    "task": "allow",
    "webfetch": "deny",
    "websearch": "deny",
    "bash": {
      "*": "allow",
      "git*": "deny",
      "gh*": "deny",
      "curl*": "deny",
      "wget*": "deny",
      "lynx*": "deny",
      "ssh*": "deny",
      "scp*": "deny",
      "sftp*": "deny",
      "rsync*": "deny",
      "nc*": "deny",
      "netcat*": "deny",
      "ncat*": "deny",
      "npx*": "deny",
      "npm*": "deny",
      "yarn*": "deny",
      "pnpm*": "deny",
      "pip*": "deny",
      "pip3*": "deny",
      "cargo publish*": "deny",
      "cargo install*": "deny",
      "cargo run*": "deny",
      "docker*": "deny",
      "podman*": "deny",
      "sudo*": "deny",
      "su*": "deny",
      "eval*": "deny",
      "exec*": "deny",
      "rm -rf /": "deny",
      "rm -rf ~*": "deny",
      "chmod 777*": "deny",
      "*/.ssh/*": "deny",
      "*id_rsa*": "deny",
      "*/.netrc*": "deny",
      "*/.git-credentials*": "deny",
      "*/.config/gh/*": "deny",
      "*/.aws/*": "deny",
      "*.pem": "deny"
    },
    "external_directory": {
      "*": "deny"
    }
  }
}
EOF
}

# run_gates — run the same verification gates as .githooks/pre-commit,
# explicitly, before any commit (F-20 R6). The false claim that a pre-commit
# hook runs these automatically was removed; ralph.sh owns the gate now.
run_gates() {
  log "Running verification gates (F-20 R6)…"
  if ! cargo fmt --manifest-path "$REPO_ROOT/figby-rs/Cargo.toml" -- --check 2>&1; then
    warn "cargo fmt --check FAILED"; return 1
  fi
  if ! cargo clippy --manifest-path "$REPO_ROOT/figby-rs/Cargo.toml" --all-targets --all-features -- -D warnings 2>&1; then
    warn "cargo clippy FAILED"; return 1
  fi
  if ! cargo build --manifest-path "$REPO_ROOT/figby-rs/Cargo.toml" 2>&1; then
    warn "cargo build FAILED"; return 1
  fi
  if ! cargo test --manifest-path "$REPO_ROOT/figby-rs/Cargo.toml" 2>&1; then
    warn "cargo test FAILED"; return 1
  fi
  good "All verification gates passed."
  return 0
}

# stage_explicit [extra paths…] — stage exactly the working-tree changes the
# agent produced (git status porcelain delta), never `git add -A` (F-20 R4).
# Aborts if any changed path looks credential-ish or git-internal. Extra
# paths (ralph-owned docs such as todo/memory/learnings) are staged on top.
stage_explicit() {
  _bad=0
  _status="$(git status --porcelain)"
  while IFS= read -r _line; do
    [ -z "$_line" ] && continue
    _path="${_line#?? }"
    case "$_path" in
      *".git/"* | ".git" | *"/.git/"*) _bad=1; warn "refusing to stage .git path: $_path" ;;
      *"/.ssh/"* | ".ssh" | ".ssh/"* | *".netrc"* | *".git-credentials"* | *".config/gh/"* | *"/.aws/"* | ".aws" | ".aws/"* | *"id_rsa"* | *".pem"*) _bad=1; warn "refusing to stage credential-ish path: $_path" ;;
    esac
    case "$_path" in
      *" -> "*) _path="${_path%% -> *} ${_path##* -> }" ;;
    esac
    git add -- $_path || _bad=1
  done <<EOF
$_status
EOF
  if [ "$_bad" -ne 0 ]; then
    die "refusing to stage changes — unexpected path(s) in working tree"
  fi
  for _p in "$@"; do
    [ -e "$_p" ] && git add -- "$_p" || true
  done
}

invoke_agent() {
  _agent="$1"
  shift
  _prompt="$1"
  shift
  _cli="$(agent_cli "$_agent")"
  _model="$(agent_model "$_agent")"

  # F-20 R1: refuse CLIs that cannot express a tool allowlist (claude/copilot
  # in this script would need --dangerously-skip-permissions / --allow-all-
  # tools, which the policy forbids). Operator may override explicitly.
  if [ "$_cli" = "claude" ] || [ "$_cli" = "copilot" ]; then
    if [ "${RALPH_ALLOW_UNSANDBOXED_CLI:-0}" != "1" ]; then
      die "agent CLI '$_cli' cannot run under the F-20 sandbox (no allowlist). Set RALPH_ALLOW_UNSANDBOXED_CLI=1 to override — at your own risk."
    fi
    warn "RALPH_ALLOW_UNSANDBOXED_CLI=1 — running '$_cli' WITHOUT an allowlist (unsandboxed, per policy escape hatch)."
  fi

  _pf="$(mktemp)"
  printf '%s' "$_prompt" >"$_pf"

  _timeout_cmd=""
  if command -v timeout >/dev/null 2>&1; then
    _timeout_cmd="timeout --foreground --kill-after=30s ${AGENT_TIMEOUT}s"
  elif command -v gtimeout >/dev/null 2>&1; then
    _timeout_cmd="gtimeout --foreground --kill-after=30s ${AGENT_TIMEOUT}s"
  fi

  if [ "$_cli" = "opencode" ]; then
    # F-20 R1/R2: sandboxed invocation — scrubbed env, isolated git config,
    # injected permissions profile (allowlist), no external plugins.
    _sb_cfg="$(mktemp)"
    write_sandbox_config "$_sb_cfg"
    _git_cfg="$(mktemp)"
    : > "$_git_cfg"
    (
      for _v in $(scrub_agent_env); do unset "$_v"; done
      export GIT_CONFIG_NOSYSTEM=1
      export GIT_CONFIG_GLOBAL="$_git_cfg"
      export OPENCODE_CONFIG="$_sb_cfg"
      export OPENCODE_PURE=1
      _cmd="cat \"$_pf\" | $_timeout_cmd \"$_cli\" run --model \"$_agent\" \"\$@\""
      if [ "$QUIET" -eq 1 ]; then
        _cmd="${_cmd} 2>/dev/null"
      fi
      eval "$_cmd"
    )
    _rc=$?
    rm -f "$_pf" "$_sb_cfg" "$_git_cfg"
    return $_rc
  fi

  # Unsandboxed escape hatch (only reachable with RALPH_ALLOW_UNSANDBOXED_CLI=1).
  _prompt_text="$(cat "$_pf")"
  rm -f "$_pf"
  if [ "$QUIET" -eq 1 ]; then
    $_timeout_cmd "$_cli" -p "$_prompt_text" --model "$_model" "$@" 2>/dev/null
  else
    $_timeout_cmd "$_cli" -p "$_prompt_text" --model "$_model" "$@"
  fi
  return $?
}

resolve_dev_agent() {
  _task_block="$1"

  _agent=$(printf '%s' "$_task_block" | grep -im1 'Agent:' | sed 's/.*Agent:\s*//' | xargs)
  if [ -n "$_agent" ]; then
    case "$_agent" in
    task_planning_agent | TASK_PLANNING_AGENT) printf '%s' "$TASK_PLANNING_AGENT" ;;
    basic_dev_agent | BASIC_DEV_AGENT) printf '%s' "$BASIC_DEV_AGENT" ;;
    mid_dev_agent | MID_DEV_AGENT) printf '%s' "$MID_DEV_AGENT" ;;
    pro_dev_agent | PRO_DEV_AGENT) printf '%s' "$PRO_DEV_AGENT" ;;
    task_review_agent | TASK_REVIEW_AGENT) printf '%s' "$TASK_REVIEW_AGENT" ;;
    release_review_agent | RELEASE_REVIEW_AGENT) printf '%s' "$RELEASE_REVIEW_AGENT" ;;
    major_release_review_agent | MAJOR_RELEASE_REVIEW_AGENT) printf '%s' "$MAJOR_RELEASE_REVIEW_AGENT" ;;
    architect_agent | ARCHITECT_AGENT) printf '%s' "$ARCHITECT_AGENT" ;;
    *) printf '%s' "$MID_DEV_AGENT" ;;
    esac
    return
  fi

  _model=$(printf '%s' "$_task_block" | grep -im1 'Model:' | sed 's/.*Model:\s*//' | xargs)
  _difficulty=$(printf '%s' "$_task_block" | grep -im1 'Difficulty:' | sed 's/.*Difficulty:\s*//' | xargs)
  _suggested=$(printf '%s' "$_task_block" | grep -im1 'Suggested model:' | sed 's/.*Suggested model:\s*//' | xargs)
  _complexity=$(printf '%s' "$_task_block" | grep -im1 'Complexity:' | sed 's/.*Complexity:\s*//' | xargs)

  case "$_model$_suggested" in
  *[Ff]lagship*)
    printf '%s' "$PRO_DEV_AGENT"
    return
    ;;
  esac
  case "$_model" in
  *[Hh]uman*)
    printf '%s' "$PRO_DEV_AGENT"
    return
    ;;
  esac

  case "$_difficulty" in
  Low)
    printf '%s' "$BASIC_DEV_AGENT"
    return
    ;;
  Medium)
    printf '%s' "$MID_DEV_AGENT"
    return
    ;;
  High)
    printf '%s' "$PRO_DEV_AGENT"
    return
    ;;
  esac

  case "$_complexity" in
  *Very\ Low* | *Low*)
    printf '%s' "$BASIC_DEV_AGENT"
    return
    ;;
  *Low-Medium* | *Medium*)
    printf '%s' "$MID_DEV_AGENT"
    return
    ;;
  *High* | *Very\ High*)
    printf '%s' "$PRO_DEV_AGENT"
    return
    ;;
  esac

  printf '%s' "$MID_DEV_AGENT"
}

next_task() {
  task_for_minor "$MINOR_VERSION"
}

next_minor() {
  all_todo_lines | grep -m1 "^\- \[ \] \`[0-9]\+\.[0-9]\+\.[0-9]\+\`" |
    sed "s/^- \[ \] \`\([^\`]*\)\`.*/\1/" |
    sed 's/\.[0-9]*$//' || true
}

switch_to_phase() {
  _minor="$1"
  _branch="release/${_minor}"

  if git show-ref --verify --quiet "refs/heads/${_branch}" 2>/dev/null ||
    git ls-remote --exit-code --heads origin "${_branch}" >/dev/null 2>&1; then
    log "Switching to existing ${_branch}"
    git checkout "$_branch" >/dev/null 2>&1
    git pull --ff-only origin "$_branch" >/dev/null 2>&1 ||
      warn "could not fast-forward ${_branch} from origin — continuing on local"
  else
    log "Creating ${_branch} from main"
    git checkout "${DEFAULT_BRANCH}" >/dev/null 2>&1
    git pull --ff-only origin "${DEFAULT_BRANCH}" >/dev/null 2>&1 ||
      warn "could not fast-forward main from origin — continuing on local"
    git checkout -b "$_branch"
    git push -u origin "$_branch"
    good "${_branch} created and pushed."
  fi

  BASE_BRANCH="$_branch"
  MINOR_VERSION="$_minor"
  log "Now on ${BASE_BRANCH}  (phase ${MINOR_VERSION})"
}

task_block() {
  TASK_ID="$1"
  all_todo_lines | awk -v tid="$TASK_ID" '
        BEGIN { pat = "^- \\[ \\] `" tid "`" }
        $0 ~ pat      { found=1; print; next }
        found && /^- \[ \] `[0-9]/ { exit }
        found         { print }
    '
}

ralph_log() {
  ENTRY="$1"
  mkdir -p "$(dirname "$LOG")"
  printf '\n## %s\n\n%s\n' "$(date '+%Y-%m-%d %H:%M')" "$ENTRY" >>"$LOG"
}

# — deadline / stop helpers —

time_remaining() {
  _now="$(date +%s)"
  _left=$((DEADLINE - _now))
  if [ "$_left" -le 0 ]; then
    printf '0s'
  else
    printf '%dh %dm %ds' $((_left / 3600)) $(((_left % 3600) / 60)) $((_left % 60))
  fi
}

deadline_reached() {
  [ "$DEADLINE" -gt 0 ] && [ "$(date +%s)" -ge "$DEADLINE" ]
}

stop_requested() {
  [ "$STOP_REQUESTED" -eq 1 ] && return 0
  if [ -f "$STOP_SENTINEL" ]; then
    warn "Stop sentinel found: $STOP_SENTINEL — consuming it."
    rm -f "$STOP_SENTINEL"
    STOP_REQUESTED=1
    return 0
  fi
  return 1
}

# — major-release gate —

is_major_release() {
  case "$MINOR_VERSION" in
  *.0) return 0 ;;
  *) return 1 ;;
  esac
}

prepare_major_rc() {
  MAJOR_VER="${MINOR_VERSION%.0}"
  RC_VER="${MAJOR_VER}.0.0"
  RC_BRANCH="rc/${RC_VER}-rc.1"
  RC_TAG="${RC_VER}-rc.1"

  log "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  log "MAJOR RELEASE — phase ${MINOR_VERSION} requires human sign-off."
  log "Auto-merge to main is BLOCKED for major releases."
  log "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

  if git show-ref --verify --quiet "refs/heads/${RC_BRANCH}" 2>/dev/null; then
    warn "RC branch ${RC_BRANCH} already exists — skipping creation."
    git checkout "$RC_BRANCH" >/dev/null 2>&1
  else
    log "Creating RC branch ${RC_BRANCH} from ${BASE_BRANCH}"
    git checkout -b "$RC_BRANCH" >/dev/null 2>&1
    git push -u origin "$RC_BRANCH"
    good "RC branch ${RC_BRANCH} created and pushed."
  fi

  if git show-ref --verify --quiet "refs/tags/${RC_TAG}" 2>/dev/null; then
    warn "RC tag ${RC_TAG} already exists — skipping."
  else
    git tag -a "$RC_TAG" -m "Release candidate: ${RC_TAG}"
    git push origin "$RC_TAG"
    good "RC tag ${RC_TAG} created and pushed."
  fi

  ralph_log "MAJOR_RC_READY: ${RC_BRANCH} + tag ${RC_TAG} created from ${BASE_BRANCH}. Awaiting human sign-off before merge to main."

  warn ""
  warn "  RC is ready: ${RC_TAG}  (branch: ${RC_BRANCH})"
  warn ""
  warn "  Required before merging to main:"
  warn "    1. Round-table flagship review (>= 2 models)"
  warn "    2. Human review and sign-off"
  warn "    3. Documented manual end-to-end test on macOS + Linux + Windows"
  warn "    4. Human runs: git checkout main/master && git merge --no-ff ${RC_BRANCH}"
  warn ""
  warn "  Ralph has stopped — do NOT re-run ralph to merge this release."
  exit 0
}

# — phase completion review + auto-merge to main —

phase_review_and_merge() {
  if is_major_release; then
    prepare_major_rc
  fi

  log "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  log "Phase ${MINOR_VERSION} complete — running review before merging to main"
  log "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

  # Hard gate: tests, clippy, and fmt must pass before LLM review or merge.
  log "Running cargo test before phase review…"
  if ! cargo test --manifest-path "$REPO_ROOT/figby-rs/Cargo.toml" 2>&1; then
    ralph_log "PHASE_BLOCKED: ${MINOR_VERSION} cargo test failed — merge aborted."
    warn "cargo test FAILED — fix all test failures before merging."
    exit 1
  fi
  log "Running cargo clippy before phase review…"
  if ! cargo clippy --manifest-path "$REPO_ROOT/figby-rs/Cargo.toml" --all-targets -- -D warnings 2>&1; then
    ralph_log "PHASE_BLOCKED: ${MINOR_VERSION} cargo clippy failed — merge aborted."
    warn "cargo clippy FAILED — fix all warnings before merging."
    exit 1
  fi
  log "Running cargo fmt check before phase review…"
  if ! cargo fmt --manifest-path "$REPO_ROOT/figby-rs/Cargo.toml" -- --check 2>&1; then
    ralph_log "PHASE_BLOCKED: ${MINOR_VERSION} cargo fmt check failed — merge aborted."
    warn "cargo fmt check FAILED — run cargo fmt before merging."
    exit 1
  fi
  good "All quality gates passed — proceeding with phase review."

  MAX_REVIEW_ATTEMPTS=3
  REVIEW_ATTEMPT=0

  while [ $REVIEW_ATTEMPT -lt $MAX_REVIEW_ATTEMPTS ]; do
    REVIEW_ATTEMPT=$((REVIEW_ATTEMPT + 1))
    log "Phase review attempt ${REVIEW_ATTEMPT}/${MAX_REVIEW_ATTEMPTS}"

    PHASE_TASK_STATUS="$(all_todo_lines | grep "\`${MINOR_VERSION}\." | head -30 || true)"
    CHANGED_FILES="$(git diff --name-only "${DEFAULT_BRANCH}"..."$BASE_BRANCH" 2>&1 || true)"
    COMMIT_LOG="$(git log --oneline "${DEFAULT_BRANCH}"..."$BASE_BRANCH" 2>&1 || true)"
    REVIEW_LOG="/tmp/ralph-phase-review-${MINOR_VERSION}-${REVIEW_ATTEMPT}.log"

    invoke_agent "$RELEASE_REVIEW_AGENT" "You are performing a phase completion review for the Figby repository.
All tasks in phase ${MINOR_VERSION} are reported complete. You have full read
access to the repository working directory. Inspect changed files and docs
directly.

Branches: main vs ${BASE_BRANCH}

Phase task status:
${PHASE_TASK_STATUS}

Commits in this phase (one per line):
${COMMIT_LOG}

Files changed in this phase:
${CHANGED_FILES}

Review checklist — for each item write PASS or FAIL and a one-line reason:
1. Every ${MINOR_VERSION}.* task in todo.md is marked [x].
2. No regressions: no test failures, compilation errors, or broken APIs introduced.
3. docs/memory.md has new entries covering this phase.
4. No unrelated scope creep — only ${MINOR_VERSION}.* owned paths changed.
5. No security issues (path traversal, secrets, unsafe writes) introduced.
6. Code quality is acceptable — no dead code, no unwraps in production paths.
7. FIGfont spec compliance: no deviations from FIGlet 2.2.5 behavior.

If every item is PASS, print exactly: PHASE_APPROVED
If any item is FAIL, print exactly: PHASE_BLOCKED
Then list what must be fixed before the merge can proceed." \
      2>&1 | tee "$REVIEW_LOG"

    ln -sf "$REVIEW_LOG" "/tmp/ralph-phase-review-${MINOR_VERSION}.log" 2>/dev/null || true

    if grep -q "PHASE_APPROVED" "$REVIEW_LOG" 2>/dev/null; then
      good "Review approved the merge — merging ${BASE_BRANCH} → main."
      # Commit any review-agent fixes before switching branches
      if ! git diff --quiet || ! git diff --cached --quiet; then
        stage_explicit docs/ralph-log.md
        git commit -m "fix: address phase ${MINOR_VERSION} review findings" || true
        git push origin "$BASE_BRANCH" 2>/dev/null || true
      fi
      git checkout "${DEFAULT_BRANCH}"
      git pull --ff-only origin "${DEFAULT_BRANCH}" >/dev/null 2>&1 ||
        warn "could not fast-forward ${DEFAULT_BRANCH} from origin — continuing on local"
      # Attempt merge; auto-resolve docs conflicts (both sides win)
      git merge --no-ff "$BASE_BRANCH" \
        -m "release: merge ${BASE_BRANCH} to ${DEFAULT_BRANCH} — phase ${MINOR_VERSION} complete" 2>&1 || true
      if [ -n "$(git ls-files --unmerged 2>/dev/null)" ]; then
        warn "Merge conflicts detected — auto-resolving docs conflicts."
        _conflicts="$(git diff --name-only --diff-filter=U 2>/dev/null || true)"
        for _f in $_conflicts; do
          case "$_f" in
          docs/*.md | CHANGELOG.md)
            # Release branch is always more current for these additive files
            git checkout --theirs "$_f"
            git add "$_f"
            good "Resolved $_f (took release branch version)."
            ;;
          *)
            warn "Non-docs conflict in $_f — manual resolution required."
            ;;
          esac
        done
        if [ -n "$(git ls-files --unmerged 2>/dev/null)" ]; then
          warn "Unresolved conflicts remain — aborting merge."
          git merge --abort 2>/dev/null || true
          ralph_log "MERGE_FAILED: ${BASE_BRANCH} → ${DEFAULT_BRANCH} blocked by unresolved conflicts."
          exit 1
        fi
        git commit --no-edit 2>/dev/null || git commit -m "release: merge ${BASE_BRANCH} to ${DEFAULT_BRANCH} — phase ${MINOR_VERSION} complete (with conflict resolution)" || true
      fi
      git push origin "${DEFAULT_BRANCH}"
      good "Phase ${MINOR_VERSION} merged to main successfully."
      ralph_log "PHASE_COMPLETE: ${MINOR_VERSION} merged to main after review approval."
      return 0
    fi

    warn "Phase review blocked (attempt ${REVIEW_ATTEMPT}/${MAX_REVIEW_ATTEMPTS}) — see ${REVIEW_LOG}"

    if [ $REVIEW_ATTEMPT -eq $MAX_REVIEW_ATTEMPTS ]; then
      warn "Phase review still blocked after ${MAX_REVIEW_ATTEMPTS} auto-fix attempts — manual intervention required."
      ralph_log "PHASE_BLOCKED: ${MINOR_VERSION} review failed after ${MAX_REVIEW_ATTEMPTS} auto-fix attempts. See ${REVIEW_LOG} for the last reviewer output. Fix manually then re-run ralph."
      exit 1
    fi

    REVIEW_OUT="$(cat "$REVIEW_LOG")"
    log "Asking architect agent to fix phase review blockers (attempt ${REVIEW_ATTEMPT})…"

    invoke_agent "$ARCHITECT_AGENT" "The phase review for phase ${MINOR_VERSION} in the Figby repository returned PHASE_BLOCKED.
Read the full review output below. Fix every issue listed under 'Must fix before merge'.
Make the minimum changes necessary — do not touch code or docs unrelated to the listed blockers.
Do NOT commit. Just edit the files. Print exactly: REVIEW_FIXES_DONE when all edits are complete.

Review output:
${REVIEW_OUT}" \
      2>&1 | tee "/tmp/ralph-review-fix-${MINOR_VERSION}-${REVIEW_ATTEMPT}.log"

    if git diff --quiet && git diff --cached --quiet; then
      warn "Fix agent made no file changes — review may still block on next attempt."
    else
      FIX_MSG="fix: address phase ${MINOR_VERSION} review blockers (attempt ${REVIEW_ATTEMPT})"
      FIX_ATTEMPT=0
      FIX_MAX=3
      FIX_COMMIT_LOG="/tmp/ralph-review-fix-commit-${MINOR_VERSION}-${REVIEW_ATTEMPT}.log"

      while [ $FIX_ATTEMPT -lt $FIX_MAX ]; do
        FIX_ATTEMPT=$((FIX_ATTEMPT + 1))
        log "Fix commit attempt ${FIX_ATTEMPT}/${FIX_MAX}"
        if ! GATE_OUT="$(run_gates 2>&1)"; then
          if [ $FIX_ATTEMPT -eq $FIX_MAX ]; then
            warn "Fix commit still failing after ${FIX_MAX} attempts — discarding unstaged changes and retrying the review anyway."
            git checkout -- . 2>/dev/null || true
            git clean -fd 2>/dev/null || true
            break
          fi
          invoke_agent "$ARCHITECT_AGENT" "The verification gates failed for the review-fix commit for phase ${MINOR_VERSION}.
Fix every failure shown below. Change only what is required to pass.
When done print exactly: FIXES_DONE.

Verification gates output (gates run explicitly before commit — F-20 R6):
${GATE_OUT}" \
            2>&1 | tee "/tmp/ralph-review-fix-gate-${MINOR_VERSION}-${REVIEW_ATTEMPT}-${FIX_ATTEMPT}.log"
          continue
        fi
        stage_explicit docs/ralph-log.md
        if git commit -m "$FIX_MSG" >"$FIX_COMMIT_LOG" 2>&1; then
          cat "$FIX_COMMIT_LOG"
          good "Verification gates passed — commit succeeded."
          git push origin "$BASE_BRANCH"
          break
        fi
        cat "$FIX_COMMIT_LOG"
        warn "git commit failed on fix commit attempt ${FIX_ATTEMPT}."
        if [ $FIX_ATTEMPT -eq $FIX_MAX ]; then
          warn "Fix commit still failing after ${FIX_MAX} attempts — discarding unstaged changes and retrying the review anyway."
          git checkout -- . 2>/dev/null || true
          git clean -fd 2>/dev/null || true
          break
        fi
        invoke_agent "$ARCHITECT_AGENT" "git commit failed for the review-fix commit for phase ${MINOR_VERSION}.
Fix the cause shown below. Change only what is required to pass.
When done print exactly: FIXES_DONE.

git output:
$(cat "$FIX_COMMIT_LOG")" \
          2>&1 | tee "/tmp/ralph-review-fix-hook-${MINOR_VERSION}-${REVIEW_ATTEMPT}-${FIX_ATTEMPT}.log"
      done
    fi
  done
}

# — per-task runner —

run_task() {
  TASK_ID="$1"
  BRANCH="task-${TASK_ID}"

  log "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  log "Starting task ${BOLD}${TASK_ID}${RESET}"
  log "Branch: ${BRANCH}  (from ${BASE_BRANCH})"
  log "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

  git checkout "$BASE_BRANCH" >/dev/null 2>&1
  git pull --ff-only origin "$BASE_BRANCH" >/dev/null 2>&1 ||
    warn "could not fast-forward from origin/${BASE_BRANCH} — continuing on local"
  if ! git checkout -b "$BRANCH" >/dev/null 2>&1; then
    ralph_log "BLOCKED on task ${TASK_ID}: branch ${BRANCH} already exists. Resolve it manually before re-running ralph."
    die "Branch '${BRANCH}' already exists — ralph cannot safely continue.\n" \
      "  To retry this task cleanly:\n" \
      "    git branch -D ${BRANCH}\n" \
      "    git push origin --delete ${BRANCH}   # if it was pushed\n" \
      "  Or mark ${TASK_ID} as done in todo.md on ${BASE_BRANCH} if the work is already complete."
  fi

  TASK_BLOCK="$(task_block "$TASK_ID")"
  SKILL_TEXT="$(cat "$SKILL")"

  # Step 1: planning agent produces a written plan
  log "Step 1/3 — planning with $(agent_model "$TASK_PLANNING_AGENT")"

  PLAN="$(
    invoke_agent "$TASK_PLANNING_AGENT" "You are an expert Rust engineer planning a task for the Figby repository.
Read the skill file and the task block carefully, then write a numbered
implementation plan. Do NOT write any code. Output plain text only.

TASK ID: ${TASK_ID}

TASK BLOCK:
${TASK_BLOCK}

SKILL FILE:
${SKILL_TEXT}

Produce:
1. A numbered list of files to create or edit (path + one-sentence purpose).
2. A numbered list of tests to write (name + what it proves).
3. Any blockers or security concerns to flag before coding starts."
  )"

  log "Plan ready."

  # Step 2: dev agent implements (resolved from task block)
  DEV_AGENT="$(resolve_dev_agent "$TASK_BLOCK")"
  log "Step 2/3 — implementing with $(agent_model "$DEV_AGENT")"

  invoke_agent "$DEV_AGENT" "You are Ralph, the autonomous task agent for the Figby repository.
Implement task ${TASK_ID} in full, following every rule in the skill file below.
All cargo commands should use: cargo \$CMD ${MANIFEST}
ralph.sh runs the verification gates (fmt, clippy, build, test) explicitly
before committing — never use --no-verify.
Do not commit yet. Write all files, then verify your work by running ONLY:
  cargo fmt ${MANIFEST} --check
  cargo clippy ${MANIFEST} --all-targets --all-features -- -D warnings
  cargo build ${MANIFEST}
  cargo test ${MANIFEST}
Fix any failures until all four pass clean.
When finished print exactly: IMPLEMENTATION_DONE.

TASK ID: ${TASK_ID}

TASK BLOCK:
${TASK_BLOCK}

PLAN:
${PLAN}

SKILL FILE:
${SKILL_TEXT}" \
    2>&1 | tee /tmp/ralph-impl-"$TASK_ID".log

  # Step 3: task review agent self-reviews the diff
  log "Step 3/3 — self-review with $(agent_model "$TASK_REVIEW_AGENT")"

  DIFF="$(git diff HEAD 2>&1 | head -600)"

  invoke_agent "$TASK_REVIEW_AGENT" "You are reviewing a Rust implementation for the Figby repository.
Work through every item in the self-review checklist from the skill file.
For each item write PASS or FAIL and a one-line reason.
For any FAIL item: open the file and fix it now before printing REVIEW_DONE.
Do not skip any checklist item.

TASK ID: ${TASK_ID}

GIT DIFF (up to 600 lines):
${DIFF}

SKILL FILE (contains the checklist):
${SKILL_TEXT}

After fixing all failures print exactly: REVIEW_DONE." \
    2>&1 | tee /tmp/ralph-review-"$TASK_ID".log

  # Commit with retry — verification gates run explicitly first (F-20 R6)
  log "Committing task ${TASK_ID}"

  COMMIT_MSG="$(
    invoke_agent "$TASK_PLANNING_AGENT" "Write a git commit message for task ${TASK_ID} in the Figby repository.
Format: first line is '${TASK_ID}: <description, 10 words max>'.
Then a blank line. Then one paragraph body: what was done and why.
Output only the commit message text, no markdown fences.
Task: ${TASK_BLOCK}"
  )"

  COMMIT_ATTEMPTS=0
  MAX_ATTEMPTS=3
  COMMIT_LOG="/tmp/ralph-commit-${TASK_ID}.log"

  while [ $COMMIT_ATTEMPTS -lt $MAX_ATTEMPTS ]; do
    COMMIT_ATTEMPTS=$((COMMIT_ATTEMPTS + 1))
    log "Commit attempt ${COMMIT_ATTEMPTS}/${MAX_ATTEMPTS}"

    if ! GATE_OUT="$(run_gates 2>&1)"; then
      warn "Verification gates failed on attempt ${COMMIT_ATTEMPTS}."
      if [ $COMMIT_ATTEMPTS -eq $MAX_ATTEMPTS ]; then
        ralph_log "BLOCKED on task ${TASK_ID}: verification gates still failing after ${MAX_ATTEMPTS} attempts."
        git checkout "$BASE_BRANCH" >/dev/null 2>&1
        git branch -D "$BRANCH" >/dev/null 2>&1 || true
        die "Giving up on ${TASK_ID} after ${MAX_ATTEMPTS} gate attempts."
      fi
      log "Asking architect agent to fix gate failures…"
      invoke_agent "$ARCHITECT_AGENT" "The verification gates failed for task ${TASK_ID} in the Figby repository.
Fix every failure shown below. Change only what is required to pass.
When done print exactly: FIXES_DONE.

Verification gates output (gates run explicitly before commit — F-20 R6):
${GATE_OUT}" \
        2>&1 | tee /tmp/ralph-fix-"$TASK_ID"-"$COMMIT_ATTEMPTS".log
      continue
    fi

    stage_explicit docs/ralph-log.md
    if git commit -m "$COMMIT_MSG" >"$COMMIT_LOG" 2>&1; then
      cat "$COMMIT_LOG"
      good "Verification gates passed — commit succeeded."
      break
    fi

    if grep -q "nothing to commit" "$COMMIT_LOG" 2>/dev/null; then
      cat "$COMMIT_LOG"
      good "Working tree is clean — commit already exists."
      break
    fi

    cat "$COMMIT_LOG"
    warn "git commit failed on attempt ${COMMIT_ATTEMPTS}."

    if [ $COMMIT_ATTEMPTS -eq $MAX_ATTEMPTS ]; then
      ralph_log "BLOCKED on task ${TASK_ID}: commit still failing after ${MAX_ATTEMPTS} attempts."
      git checkout "$BASE_BRANCH" >/dev/null 2>&1
      git branch -D "$BRANCH" >/dev/null 2>&1 || true
      die "Giving up on ${TASK_ID} after ${MAX_ATTEMPTS} commit attempts."
    fi

    HOOK_OUT="$(cat "$COMMIT_LOG")"
    log "Asking architect agent to fix failures…"
    invoke_agent "$ARCHITECT_AGENT" "git commit failed for task ${TASK_ID} in the Figby repository.
Fix the cause shown below. Change only what is required to pass.
When done print exactly: FIXES_DONE.

git commit output:
${HOOK_OUT}" \
      2>&1 | tee /tmp/ralph-fix-"$TASK_ID"-"$COMMIT_ATTEMPTS".log
  done

  git push origin HEAD

  good "Task ${TASK_ID} committed and pushed on branch ${BRANCH}."

  # Mark task done in todo file before merging
  log "Marking ${TASK_ID} done in todo file..."
  _todo_files="$REPO_ROOT/docs"/todo-v*.md
  _todo_match="$(grep -l "\`${TASK_ID}\`" $_todo_files 2>/dev/null | head -1)"
  if [ -n "$_todo_match" ]; then
    if grep -q "\- \[ \] \`${TASK_ID}\`" "$_todo_match" 2>/dev/null; then
      sed -i "s/^- \[ \] \`${TASK_ID}\`/- [x] \`${TASK_ID}\`/" "$_todo_match"
      git add "$_todo_match"
      git commit -m "docs: mark ${TASK_ID} done"
      git push origin HEAD
      good "Task ${TASK_ID} checked off in todo."
    else
      good "Task ${TASK_ID} already checked off — skipping."
    fi
  else
    warn "Could not find todo file for task ${TASK_ID} — checkbox not updated."
  fi

  # Hard gate: tests must pass before merging task branch into release branch.
  log "Running cargo test before task merge…"
  if ! cargo test --manifest-path "$REPO_ROOT/figby-rs/Cargo.toml" 2>&1; then
    ralph_log "TASK_BLOCKED: ${TASK_ID} cargo test failed — task merge aborted."
    warn "cargo test FAILED for ${TASK_ID} — fix failures before merging."
    exit 1
  fi

  # Merge back into release branch
  log "Merging ${BRANCH} back into ${BASE_BRANCH}"
  git checkout "$BASE_BRANCH"
  git pull --ff-only origin "$BASE_BRANCH" >/dev/null 2>&1 ||
    warn "could not fast-forward ${BASE_BRANCH} from origin — continuing on local"
  git merge --no-ff "$BRANCH" -m "merge: ${BRANCH} into ${BASE_BRANCH}"
  git push origin "$BASE_BRANCH"

  git branch -d "$BRANCH"
  git push origin --delete "$BRANCH" 2>/dev/null || true

  good "Task ${TASK_ID} merged into ${BASE_BRANCH} and task branch cleaned up."
  ralph_log "DONE: ${TASK_ID} merged into ${BASE_BRANCH}."
}

# — main —

if [ -n "$SINGLE_TASK" ]; then
  if deadline_reached || stop_requested; then
    warn "Stop/deadline condition met before task could start."
    exit 0
  fi
  if [ "$BASE_BRANCH" = "main" ]; then
    _task_minor="$(printf '%s' "$SINGLE_TASK" | sed 's/\.[0-9]*$//')"
    switch_to_phase "$_task_minor"
  fi
  run_task "$SINGLE_TASK"
  exit 0
fi

TASKS_DONE=0

while true; do
  if deadline_reached; then
    good "Time limit reached. Tasks completed this session: ${TASKS_DONE}."
    ralph_log "Time limit reached. Tasks completed: ${TASKS_DONE}."
    exit 0
  fi

  if stop_requested; then
    good "Graceful stop. Tasks completed this session: ${TASKS_DONE}."
    ralph_log "Graceful stop. Tasks completed: ${TASKS_DONE}."
    exit 0
  fi

  if [ "$BASE_BRANCH" = "main" ]; then
    git pull --ff-only origin "${DEFAULT_BRANCH}" >/dev/null 2>&1 ||
      warn "could not fast-forward ${DEFAULT_BRANCH} from origin — continuing on local"
    _next_minor="$(next_minor)"
    if [ -z "$_next_minor" ]; then
      good "All phases complete. Tasks completed this session: ${TASKS_DONE}."
      ralph_log "All phases complete. Tasks completed: ${TASKS_DONE}."
      exit 0
    fi
    switch_to_phase "$_next_minor"
  fi

  TASK_ID="$(next_task)"

  if [ -z "$TASK_ID" ]; then
    good "All ${MINOR_VERSION} tasks complete (${TASKS_DONE} done this session)."
    ralph_log "All ${MINOR_VERSION} tasks complete. Starting phase review."
    phase_review_and_merge
    BASE_BRANCH="main"
    MINOR_VERSION=""
    continue
  fi

  run_task "$TASK_ID" || {
    warn "Task ${TASK_ID} failed — logging and moving on."
    ralph_log "FAILED: ${TASK_ID} — see /tmp/ralph-*.log for details."
    git checkout "$BASE_BRANCH" >/dev/null 2>&1 || true
    git branch -D "task-${TASK_ID}" >/dev/null 2>&1 || true
  }

  TASKS_DONE=$((TASKS_DONE + 1))

  if [ -n "$UNTIL_TASK" ] && [ "$TASK_ID" = "$UNTIL_TASK" ]; then
    good "Reached --until=${UNTIL_TASK}. Tasks completed this session: ${TASKS_DONE}."
    ralph_log "UNTIL_STOP: reached ${UNTIL_TASK} after ${TASKS_DONE} tasks."
    exit 0
  fi

  if [ "$DEADLINE" -gt 0 ]; then
    log "Time remaining: $(time_remaining)"
  fi

  sleep 2
done
