# UI-A progress (app/**)

## Commits

- 9fae1d6 U1 — Settings pages (projects, tags, metrics), item editor project picker.

## Current step

U3 — Stats rebuilt with charts and a working filter.

## Next step

Final report.

## U1 (done)

- Settings has General (sleep time + prayer calculation restored from c673a99, plus Asr,
  coordinates, location source), Projects, Tags, Metrics sub-pages (list + editor each).
- Projects editor: name, colour swatches + hex, note, tag chips, per-metric Ask/Required,
  archive/restore, show archived. Tags: name, colour, rename, remove (confirm). Metrics:
  name, unit, kind, scale max, rollup (fixed after create), archive; built-ins locked.
- Item editor: project chips (archived hidden) + inline "Create project" that selects the new
  project; planned minutes with 15/30/60 chips. ItemDraft now carries projectId/plannedMinutes.
- Bug fixed: projects.list/tags.list/metrics.list shared one read-coalescing key, so tags and
  metrics received the projects list.
- Tests render with the Material style (as main.cpp). Removed the app test's assertions on
  plugin/local.salah/SessionPanel.qml internals (UI-W is rewriting it; the plugin has its own
  checks).
- Screenshots: SALAH_UI_SHOT_DIR=<dir> app/bin/test (trackingUiScreens).

## U2 (done, follows "User decisions, 2026-11-23")

- Timer = play/pause: Pause sends `session.stop` with no values, no prompt. Running card at the
  top of the right column (from `status.get.running`, refreshed on every reload/`changed`).
- Done = explicit tick: the done box opens the done panel (project metrics or Focus/Quality,
  note; all optional; Done / Skip) and sends a tick `session.log`; a running timer on the item
  is paused first. Done again removes the tick sessions only. `done` comes from day.get.
- Each item lists its sessions (expand "N sessions"): time, minutes, values, note; Edit (minutes,
  metrics, note; ticks edit values/note only) and Remove (confirm). "+ time" logs minutes with
  15–90 quick chips and optional metrics.
- Settings project editor: no Required toggle; asked metrics listed with Remove, others added
  with "+ name" chips; `required` is always sent false.
- Backend: status.get in reload (4 requests per reload now; tests updated), session.edit /
  session.remove / filtered stats in CoreClient, JSON-RPC error data kept, empty item_id no
  longer sent on project-only logs, day fulfilment ratio derived from the two sums.
- Removed unused CounterStepper/CorrectionBox and adjustMinutes/adjustCount (adjustCount
  logged a tick per tap).

## Notes

- Baseline before U1: `app/bin/test` 45 passed (43 test functions + init/cleanup).
