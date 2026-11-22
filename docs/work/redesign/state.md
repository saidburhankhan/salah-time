# State: redesign

## Repository

- Branch: `items-mvp`
- Uncommitted: none

## Now

User feedback round 2 (2026-11-22): drag and drop behaved like a free board,
needed in the widget too; items needed before Fajr; hide the sleep setting in
the widget; everything must feel fast. Core for the night block is committed
(fe6419b): `night` is the coming day's first block (sleep → Fajr). The CLI
answers in ~2 ms, so responsiveness work is in the UIs
(`interaction.md`). Codex workers W2 (widget) and A4 (app) are implementing
interaction.md and the night block.

## Next

1. Review W2/A4 diffs and screenshots; commit; install.
2. User restarts the shell; live-check the widget: drag, quick add, ticks,
   prayer marks, night block.
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
