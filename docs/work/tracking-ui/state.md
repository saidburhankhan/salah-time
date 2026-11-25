# State: tracking-ui

## Repository

- Branch: `main` (merged from `architecture` on 2026-11-25; fast-forward).
- Uncommitted: none.
- Live install: 2026-11-25 18:10, DB schema v7 (backups v4–v6 kept), daemon `salah.service`
  on the user socket, widget `local.salah` loads cleanly.

## Now

All planned slices and every user addendum in `plan.md` are shipped and installed. The newest
features (25 Nov) are: multi-metric Trends chart; archiving a project pauses its goals, routines
and open tasks; Insights metric filters (incl. distractions per hour); a separate Notes page with
one row per note; editable timed notes; click-to-edit notes in the app and widget; Esc cancels and
Enter saves in every dialog, and Esc closes the widget one layer at a time.

## Next

- User live check of the 25 Nov features, especially in the real widget: click-to-edit notes,
  Enter/Shift+Enter/Esc, Esc closing the popup, archive → paused goals/routines → unarchive.
  Record and fix only observed failures.
- Open question for the user: remove the Statistics "Notes" tab (whole sessions) now that the
  Notes page exists?
- Deferred, not started: none queued.

## Verification (last install, 2026-11-25 18:10)

`scripts/test && app/bin/test && ./scripts/install`: 273 cargo tests (incl. 8 daemon socket tests),
104 app tests (0 skipped), widget node checks; live shell journal clean; bar "Isha 7m".

## Workers and tooling

- Codex workers C8 (backend), C9 (app), C10 (widget), D1 (docs) via
  `~/.cache/salah-agents/scripts/dispatch-codex.sh` (logs in `~/.cache/salah-agents/`). Codex runs
  `-s workspace-write`: socket tests skip there, so the lead runs them before committing.
- Protocol doc moved to `docs/protocol-v1.md` (older briefs say docs/work/architecture/…); tell
  resumed backend workers.
- Model tiers: Sol by default, Astra only for hard design/data-engine work.
- Install only through `scripts/test && app/bin/test && ./scripts/install`; then check the live
  shell journal for `local.salah` failures and the bar.

Session history: `history.md`.
