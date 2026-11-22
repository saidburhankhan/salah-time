# State: redesign

## Repository

- Branch: `items-mvp`
- Uncommitted: none

## Now

Round 3 committed and installed (b747ea2, core f960921): timers on every item
(pause/continue), − / + adjustments (count ±1, otherwise minutes ±5, totals
clamped at 0), running timer in the bar instead of the open count, Esc
handling, quick-add Cancel, inline edit/delete in the widget, next-occurrence
hints, and future routines listed in stats. Widget unverified live.

## Next

1. User restarts the shell and checks the widget (timers, ±, edit/delete,
   Esc, drag).
2. Statistics redesign by date range, then tags (new work item).
3. Merge `sleep-boundary` and `items-mvp` into `main` once accepted.

## Interfaces

- `docs/work/redesign/contract.md` is the CLI contract for app and widget.

## Risks and blockers

- Drag and drop is tested only at the command level, not by pointer.
- The running shell caches plugin components; a restart is needed to see
  widget changes.

## Verification (2026-11-22)

- `cargo test --offline`: 96 passed. Strict Clippy clean.
- `app/bin/test`: 8 passed. `omarchy plugin validate plugin/local.salah`: ok.
- Contract exercised end to end through `target/debug/salah` in a temp store.
