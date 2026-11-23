# C9 progress

## 0-ui

- Statistics orders the filters Range, Projects, Epics, Tasks, Tags and Metrics. Selecting a tag keeps the project selection. “No project” stays visible with a tag selected in Statistics and Backlog. The Statistics task search matches the visible height of the chips.
- The project editor labels the Tags section “Default tags for new tasks”. Backlog project headings use the same catalog colours as the project chips and fall back to the row colour when the catalog is unavailable.
- `app/bin/build`, `SALAH_UI_SHOT_DIR=/tmp/salah-c9-0-ui-reviewed app/bin/test`, and `git diff --check` passed. Qt reported 40 passed and 37 skips due to the Unix socket restriction. Reviewed all generated PNGs, including `stats-task-tag-filters.png`, `settings-project-default-tags.png` and `backlog-project-dots.png`. A socket-free fixture test covers the filter order, search and chip heights, project selection and heading colours.

CHECKPOINT 0-ui: Reorder Statistics filters, keep project choices independent of tags, label project defaults and match Backlog heading colours (`app/src/qml/pages/BacklogPage.qml`, `app/src/qml/pages/StatisticsPage.qml`, `app/src/qml/settings/ProjectsSettings.qml`, `app/src/qml/statistics/FilterBar.qml`, `app/tests/salah_app_tests.cpp`, `docs/work/tracking-ui/c9-progress.md`)
