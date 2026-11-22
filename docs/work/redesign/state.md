# State: redesign

## Repository

- Branch: `items-mvp`
- Uncommitted: none

## Now

Round 4 committed and installed (cff85f7, core 778b182): every item tracks
time (timer chip + typed correction box), counters show a stepper, the form
offers Task/Counter with an optional goal, weekly needs a day, stop accepts an
id. Stats now carry `daily` and `totals` (2fe8266). Codex A8 is redesigning
the Statistics page (range bar, summary tiles, separate daily charts, table by
day, per-task breakdown).

## Next

1. Review A8 diff and screenshots against real seeded data; commit; install.
2. User restarts the shell and checks the widget (time chip, correction box,
   stepper, edit/delete, Esc, drag).
3. Tags (after statistics), then merge into `main`.

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
