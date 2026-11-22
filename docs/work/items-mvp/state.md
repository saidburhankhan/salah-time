# State: items-mvp

## Repository

- Branch: `items-mvp` (from `sleep-boundary`)
- Uncommitted: `plugin/local.salah/` (S6 widget, awaiting live check);
  `app/` and `.gitignore` (S7, Codex worker in progress)

## Now

S1–S5 committed: clap CLI, schedule/day keys, item/entry domain, SQLite
store, export/import, planner use cases, `salah item|history|export|import|open`.
Installer bug fixed: `scripts/test` was disabling the live bar widget through
`uninstall` → `omarchy plugin disable` (acts on the live shell whatever HOME
is). The widget code (S6) is installed, but the running shell still serves a
cached old component.

## Next

1. User runs `omarchy restart shell`; then screenshot the popup
   (`omarchy-shell shell summon local.salah ""`, `grim`) and check the journal.
2. Review the S7 app worker's diff, build, tests, and screenshots.
3. Install the app (`salah-app`, desktop entry, icon) from `scripts/install`.

## Interfaces

- `status --json` keeps every existing field; additions: `schema_version`,
  `day`, `current_block.block`, `planner`, `planner_error`.
- Planner CLI contract: see the S5 command list in `plan.md` / `salah --help`.
- Workers own only the files listed in their brief.

## Risks and blockers

- DeepSeek headless needs `DEEPSEEK_API_KEY` exported from the dsh credential
  store (wrapper script in the session scratchpad).
- Live shell caches plugin components; structural QML changes need a shell
  restart.

## Verification (2026-11-22)

- `./scripts/test`: passed (59 unit, 8 CLI, 3 config, 4 export, 6 planner CLI
  tests, installer checks, plugin validation).
- `cargo clippy --offline --all-targets -- -D warnings`: passed.
- Manual CLI run in a temporary store: add/list/log/timer/status/history/export
  and error cases behave as specified.
- Live bar stays enabled after `./scripts/test` and after a temporary-home
  uninstall.
