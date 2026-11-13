#!/bin/bash
# ralph-monitor.sh — detects rate-limit events and switches to paid provider.
# Run every 15 min via cron. Idempotent after switch.

REPO="/home/user/git_repos/Figby"
LOG="$REPO/docs/ralph-log.md"
RALPH_PID_FILE="/tmp/ralph.pid"
STOP_FILE="$REPO/scripts/STOP.md"
STATE_FILE="/tmp/ralph-monitor.state"
PAID_PROVIDER="opencode-go/deepseek-v4-flash"

ts() { date '+%Y-%m-%d %H:%M:%S'; }

# Already switched — nothing to do
if [ -f "$STATE_FILE" ] && grep -q "switched" "$STATE_FILE" 2>/dev/null; then
    exit 0
fi

# Ralph not running — nothing to do
RALPH_PID=""
if [ -f "$RALPH_PID_FILE" ]; then
    RALPH_PID="$(cat "$RALPH_PID_FILE" 2>/dev/null)"
fi
if [ -z "$RALPH_PID" ] || ! kill -0 "$RALPH_PID" 2>/dev/null; then
    exit 0
fi

# Check last 100 log lines for rate-limit signals
if ! tail -100 "$LOG" 2>/dev/null | grep -qiE "rate.?limit|429|too many request|quota.?exceed|rate_limit|context_length_exceeded|capacity"; then
    exit 0
fi

echo "$(ts): [monitor] rate limit detected — switching to $PAID_PROVIDER" >> "$LOG"

# Signal ralph to stop cleanly after current task
touch "$STOP_FILE"
echo "$(ts): [monitor] STOP sentinel placed — waiting for ralph to finish task" >> "$LOG"

# Wait up to 10 minutes for ralph to exit
WAIT=0
while kill -0 "$RALPH_PID" 2>/dev/null && [ "$WAIT" -lt 600 ]; do
    sleep 15
    WAIT=$((WAIT + 15))
done

if kill -0 "$RALPH_PID" 2>/dev/null; then
    echo "$(ts): [monitor] ralph did not stop within 10 min — force killing" >> "$LOG"
    kill -TERM "$RALPH_PID" 2>/dev/null
    sleep 5
fi

rm -f "$STOP_FILE"

# Update ralph.sh defaults to paid provider (sed in-place)
sed -i "s|opencode/deepseek-v4-flash-free|$PAID_PROVIDER|g" "$REPO/scripts/ralph.sh"
echo "$(ts): [monitor] ralph.sh updated to $PAID_PROVIDER" >> "$LOG"

# Restart ralph with paid provider env vars (explicit override in case script cache)
cd "$REPO" || exit 1
export TASK_PLANNING_AGENT="$PAID_PROVIDER"
export BASIC_DEV_AGENT="$PAID_PROVIDER"
export MID_DEV_AGENT="$PAID_PROVIDER"
export PRO_DEV_AGENT="$PAID_PROVIDER"
export TASK_REVIEW_AGENT="$PAID_PROVIDER"
export RELEASE_REVIEW_AGENT="$PAID_PROVIDER"
export MAJOR_RELEASE_REVIEW_AGENT="$PAID_PROVIDER"
export ARCHITECT_AGENT="$PAID_PROVIDER"

nohup ./scripts/ralph.sh >> "$LOG" 2>&1 &
echo "$(ts): [monitor] ralph restarted with $PAID_PROVIDER (PID $!)" >> "$LOG"

echo "switched:$PAID_PROVIDER:$(ts)" > "$STATE_FILE"
