# UI-A progress (app/**)

## Commits

(none yet)

## Current step

U2 — Today: running timer + stop prompt, session list edit/remove, + time, tick/untick.

## Next step

U3 — Stats rebuilt with charts and a working filter.

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

## Notes

- Baseline before U1: `app/bin/test` 45 passed (43 test functions + init/cleanup).
