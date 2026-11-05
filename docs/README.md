# docs/ index

What each file here is, so readers can tell user docs from agent working
files. Paths under "Agent memory and logs" are hard-coded in
`scripts/ralph.sh`, `skills/ralph.md` and `AGENTS.md`, so they stay where
they are.

## Start here

| File | What |
|---|---|
| [full-review-2026-11-04.md](full-review-2026-11-04.md) | Latest full review, with a live **Fix status** table |
| [todo.md](todo.md) | Milestone index (which `todo-vN.md` is active) |
| [feature-inventory.md](feature-inventory.md) | What Figby does, by area |
| [recording-guide.md](recording-guide.md) | Driving and recording the TUI (tmux, MCP server) |

## Design specs

[lighting-design.md](lighting-design.md),
[particles-design.md](particles-design.md),
[figby-tui-timelapse-storyboard.md](figby-tui-timelapse-storyboard.md),
[f-20-ralph-sandbox-policy.md](f-20-ralph-sandbox-policy.md) (draft for sign-off)

## Milestone task lists

[todo-v1.md](todo-v1.md) … [todo-v8.md](todo-v8.md), [todo.json](todo.json).
Status per milestone is in [todo.md](todo.md).

## Reviews and audits (oldest first)

Each is a snapshot. A later review doesn't cancel an earlier one's fix list:
items checked off there stay fixed, and open ones were re-checked by the
next full review.

| File | Date | Scope |
|---|---|---|
| [tui-arch-audit.md](tui-arch-audit.md) | 2026-06-17 | TUI architecture vs ratatui practice |
| [animation-audit.md](animation-audit.md) | 2026-06-18 | Animation 4.5–4.8 vs spec (marked superseded 2026-07-08) |
| [codebase-audit-2026-06-18.md](codebase-audit-2026-06-18.md) | 2026-06-18 | Whole codebase |
| [sonnet5-review.md](sonnet5-review.md) | 2026-07-08 | Whole codebase |
| [review-remediation-checklist.md](review-remediation-checklist.md) | 2026-08-20 | Fix list for the GPT review (the `F-NN` ids in code comments) |
| [OxAlphaReview.md](OxAlphaReview.md) | 2026-08-26 | Whole repository |
| [ratatui_cleanup.md](ratatui_cleanup.md) | 2026-10-28 | Widget reuse |
| [full-review-2026-11-04.md](full-review-2026-11-04.md) | 2026-11-04 | Whole codebase, C parity, CI — **latest** |

## Manual and end-to-end test checklists

- E2E: [e2e-tui-testing-plan.md](e2e-tui-testing-plan.md) (plan), then
  [e2e-tui-basics.md](e2e-tui-basics.md),
  [e2e-tui-font-editor.md](e2e-tui-font-editor.md),
  [e2e-tui-image-editor.md](e2e-tui-image-editor.md),
  [e2e-tui-file-ops.md](e2e-tui-file-ops.md),
  [e2e-tui-ui-polish.md](e2e-tui-ui-polish.md),
  [e2e-tui-art-tests.md](e2e-tui-art-tests.md),
  [e2e-cli-info.md](e2e-cli-info.md),
  [e2e-image-pipeline.md](e2e-image-pipeline.md),
  [e2e-template.md](e2e-template.md),
  [e2e-test-checklist.md](e2e-test-checklist.md) (`--create-font`)
- Manual notes: [manual-testing-v7.md](manual-testing-v7.md) (latest),
  [5.0-manual-testing-notes.md](5.0-manual-testing-notes.md),
  [4.0-manual-testing-notes.md](4.0-manual-testing-notes.md)

## Handovers

[handover-file-dialogs.md](handover-file-dialogs.md),
[handoff-2026-06-22.md](handoff-2026-06-22.md) (v6 session; its test-failure
count is stale, see the 2026-06-18 audit)

## Agent memory and logs

Working files for the autonomous loop, large and append-only. Not user
documentation.

| File | What |
|---|---|
| [memory.md](memory.md) | Cross-cutting decisions (read by `skills/ralph.md`) |
| [learnings.md](learnings.md) | Surprises and gotchas |
| [ralph-log.md](ralph-log.md) | Loop run log (written by `scripts/ralph.sh`) |
| [memory-v1.md](memory-v1.md), [memory-v2.md](memory-v2.md), [memory-v3.md](memory-v3.md), [memory-v5.md](memory-v5.md) | Per-milestone memory archives |
