# State: redesign

## Repository

- Branch: `items-mvp`
- Uncommitted: none

## Now

Core, CLI, app (three polish rounds), widget, and README are committed and
installed (6389169). The app was verified offscreen against the real CLI.
The live widget still runs a cached old component (labels showed raw i18n
keys); it needs `omarchy restart shell` before it can be checked.

## Next

1. After the user restarts the shell: screenshot the widget popup
   (`omarchy-shell shell summon local.salah ""`, `grim`), check the journal,
   and try quick add, a prayer mark, and an item tick live.
2. Test drag and drop by pointer in the installed app.
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
