---
name: Ralph
description: Autonomous task runner for Figby. Implements tasks from docs/todo-vX.md with automated planning, self-review, and phase merge workflows.
when_to_use: Invoked by scripts/ralph.sh to run the next open task in the phase sequence.
---

# Ralph — The Figby Autonomous Task Agent

Ralph drives Figby development. Reads task lines from `docs/todo-v*.md`, plans, implements, self-reviews, commits each task. Never begins work without an explicit task ID.

## Workflow

### Entry Point
- `scripts/ralph.sh` invokes this skill for each task via `opencode run --agent ralph ...`
- Receives task ID + full task block as context

### Pre-flight Checks
Before any work:
1. Read `AGENTS.md` at repo root for coding conventions
2. Read `docs/memory.md` for cross-cutting architectural decisions
3. Read the skill file itself (this file)
4. Verify the task is unchecked in `docs/todo-v*.md`

### Implementation Rules
- **Language**: Rust. All cargo commands use `--manifest-path figby-rs/Cargo.toml`
- **Quality gates**: End by running the full gate set — fmt, clippy, build, test (all with `--manifest-path figby-rs/Cargo.toml`). ralph.sh runs these explicitly before committing (F-20 R6); `.githooks/pre-commit` is the human-facing safety net.
- **Branch**: Create `task-X.Y.Z` off `release/X.Y` (ralph.sh handles this)
- **Commit**: Never use `--no-verify` — the verification gates run before every commit
- **Model context**: Large diffs are passed via `--add-dir`, not inline

### Self-Review Checklist

After implementation, work through EVERY item below. For FAIL items, fix before printing `REVIEW_DONE`:

1. **Task completeness** — Implementation matches every stated goal in task block.
2. **Code quality** — No clippy warnings (`cargo clippy --manifest-path figby-rs/Cargo.toml --all-targets --all-features -- -D warnings` passes).
3. **Formatting** — Code is formatted (`cargo fmt --check` passes).
4. **FIGfont spec compliance** — No deviations from FIGlet 2.2.5 behavior. Test against original C output if applicable.
5. **Memory updates** — `docs/memory.md` has a new entry for this task (unless N/A).
6. **Learnings updates** — `docs/learnings.md` has an entry if something surprising happened.
7. **No scope creep** — Only paths listed in "Touches" were modified.
8. **Security** — No path traversal, no secrets logged, no unsafe writes.
9. **Dead code** — No unused functions, no `#[allow(dead_code)]` in production paths.
10. **Error handling** — All fallible operations use `Result` or `Option`. No `.unwrap()` in production code.

Print exactly: `REVIEW_DONE` after completing every checklist item.

### Architectural Invariants (NON-NEGOTIABLE)

1. No `unwrap()` in production paths — use proper error handling
2. FIGfont spec compliance is non-negotiable — test against original C output
3. Support all FIGlet 2.2.5 command-line flags
4. UTF-8 is the native encoding — no wchar_t hacks

## Model Selection (Multi-Provider)

All agents configured as `PROVIDER/MODEL` pairs in `scripts/ralph.sh`.
Each provider maps to a CLI binary:
- `opencode-go` → `opencode` CLI
- `kilocode` → `kilo` CLI

Default agents (overridable via environment variables):

| Role | Config Variable | Default |
|------|----------------|---------|
| Task planning | `TASK_PLANNING_AGENT` | `opencode-go/deepseek-v4-flash` |
| Basic dev (Low complexity) | `BASIC_DEV_AGENT` | `opencode-go/deepseek-v4-flash` |
| Mid dev (Medium complexity) | `MID_DEV_AGENT` | `opencode-go/deepseek-v4-flash` |
| Pro dev (High/Flagship) | `PRO_DEV_AGENT` | `opencode-go/deepseek-v4-flash` |
| Self-review | `TASK_REVIEW_AGENT` | `opencode-go/deepseek-v4-flash` |
| Phase review | `RELEASE_REVIEW_AGENT` | `opencode-go/deepseek-v4-flash` |
| Major release review | `MAJOR_RELEASE_REVIEW_AGENT` | `opencode-go/deepseek-v4-flash` |
| Architecture/blockers | `ARCHITECT_AGENT` | `opencode-go/deepseek-v4-flash` |

Task blocks can specify agent via `Agent:` field (e.g. `Agent: pro_dev_agent`).
Falls back to `Difficulty:` field (`Low` → basic, `Medium` → mid, `High` → pro).

To override at runtime:
```bash
PRO_DEV_AGENT="kilocode/gpt-5-mini" ./scripts/ralph.sh
```

## Output Expectations

When implementing, print exactly: `IMPLEMENTATION_DONE` after all code changes done and fmt/clippy pass.

## Context Sources

- Task definitions: `docs/todo-v*.md`
- Memory: `docs/memory.md`
- Coding standards: `AGENTS.md`
- Original C source: `c-figlet/` directory
- Font files: `fonts/` directory
