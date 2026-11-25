# State: items-mvp

## Repository

- Branch: `items-mvp` (from `sleep-boundary`)
- Uncommitted: none

## Now

S1–S7 committed and installed: CLI, data core, widget, and the `salah-app`
desktop app (`salah open`, launcher entry "Salah"). The app was verified
offscreen against the real CLI with seeded data. The widget's live view is
unverified because the running shell still serves a cached old component.

## Next

1. User runs `omarchy restart shell`; then screenshot the popup
   (`omarchy-shell shell summon local.salah ""`, `grim`) and check the journal.
2. Live evening check: after Isha the bar counts down to Sleep; after sleep
   time it shows `Sleeping · Fajr`.
3. Follow-up: make the app's CLI calls asynchronous (they block the window
   for up to 10 s if the CLI hangs).

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
