# State: redesign

## Repository

- Branch: `items-mvp`
- Uncommitted: none

## Now

Round 2 committed and installed (ebefdc8): night block, live drag and drop
in app and widget, optimistic updates, no rebuild on unchanged data, widget
without the sleep-time field. App drag verified with an offscreen harness that
drives beginDrag/updateDrag/finishDrag and reads back `salah day --json`
(two bugs found and fixed: window-to-item mapping threw; ListModel rows reached
C++ as empty maps so drops were never saved). Widget drag is unverified live.

## Next

1. User restarts the shell and tries the widget: drag (handle and
   long-press), quick add, ticks, prayer marks, Before Fajr.
2. Turn the drag harness into an app test so a broken drop fails `bin/test`.
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
