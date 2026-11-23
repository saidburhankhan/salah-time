# C9 progress

## 0-ui

- Statistics orders the filters Range, Projects, Epics, Tasks, Tags and Metrics. Selecting a tag keeps the project selection. “No project” stays visible with a tag selected in Statistics and Backlog. The Statistics task search matches the visible height of the chips.
- The project editor labels the Tags section “Default tags for new tasks”. Backlog project headings use the same catalog colours as the project chips and fall back to the row colour when the catalog is unavailable.
- `app/bin/build`, `SALAH_UI_SHOT_DIR=/tmp/salah-c9-0-ui-reviewed app/bin/test`, and `git diff --check` passed. Qt reported 40 passed and 37 skips due to the Unix socket restriction. Reviewed all generated PNGs, including `stats-task-tag-filters.png`, `settings-project-default-tags.png` and `backlog-project-dots.png`. A socket-free fixture test covers the filter order, search and chip heights, project selection and heading colours.

CHECKPOINT 0-ui: Reorder Statistics filters, keep project choices independent of tags, label project defaults and match Backlog heading colours (`app/src/qml/pages/BacklogPage.qml`, `app/src/qml/pages/StatisticsPage.qml`, `app/src/qml/settings/ProjectsSettings.qml`, `app/src/qml/statistics/FilterBar.qml`, `app/tests/salah_app_tests.cpp`, `docs/work/tracking-ui/c9-progress.md`)

## 0-data

- Today and Backlog task editors show project default tags for new tasks. Changing the project on a new draft reloads its defaults, matching the widget. Existing tasks keep their tag selection across project changes. Untouched tags are omitted from writes, and an explicit empty list clears the tags.
- Statistics offers tags from tasks in the current project and epic scope, all tags in the all-projects view, and “No tag”. Tag selection narrows the task picker and retains the project. Backlog uses the task-tag filter on `backlog.list` and labels each task with its own tags. Settings counts live tasks and projects using each tag.
- Tests exercise explicit and inherited writes, projectless tags, “No tag”, and the Settings counts. Fixture coverage uses `item_add_tags.json`, `items_list.json`, `backlog_list.json`, and `stats_table_tag_items.json`.
- `app/bin/build`, `SALAH_UI_SHOT_DIR=/tmp/salah-c9-tags-final app/bin/test`, and `git diff --check` passed. Qt reported 41 passed and 37 skips due to the Unix socket restriction. Reviewed all 12 generated PNGs. The socket suite also captures the Backlog with a tag and “No project” selected together.

CHECKPOINT 0: Add task tags to the app editors, Statistics, Backlog and Settings (`app/src/backend.cpp`, `app/src/client/coreclient.cpp`, `app/src/client/coreclient.h`, `app/src/models/viewstate.cpp`, `app/src/models/viewstate.h`, `app/src/qml/backlog/BacklogRow.qml`, `app/src/qml/overlays/EntryFields.qml`, `app/src/qml/overlays/EntryForm.qml`, `app/src/qml/pages/BacklogPage.qml`, `app/src/qml/settings/TagsSettings.qml`, `app/src/qml/statistics/FilterBar.qml`, `app/tests/salah_app_tests.cpp`, `app/tests/tracking_fixture.h`, `docs/work/tracking-ui/c9-progress.md`)
