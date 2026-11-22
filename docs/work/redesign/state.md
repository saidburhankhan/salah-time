# State: redesign

## Repository

- Branch: `items-mvp`
- In progress (uncommitted while workers run): `app/**` (Codex polish
  round A2), `README.md` (DeepSeek)

## Now

C1/C2 committed (d4afad2): weekly repeat, item position + move, prayer
marks, quick-add parser, day plan, stats, export v2, store migration 2,
CLI per `contract.md`. A1/W1 committed (8efcff4): redesigned app and widget.
App verified offscreen against the real CLI with seeded data; widget passes
validation but has not been seen live yet.

## Next

1. Review the A2 polish diff and screenshots; commit.
2. Review the README update; commit.
3. `./scripts/install`, then ask the user for `omarchy restart shell`;
   screenshot the widget popup (`omarchy-shell shell summon local.salah ""`,
   `grim`) and check the journal.

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
