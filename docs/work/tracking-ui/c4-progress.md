# C4 progress

## Slice 1

- All three Today pause controls open the optional session panel. Worked starts at the rounded elapsed time when the panel opens. The daemon validates duration overrides. Save & Done is available for item sessions and uses one `session.stop` write.
- Implemented against `plan.md`. `session_stop_done.json` and `session_stop_minutes.json` appeared in the backend worktree during verification and match the expected result shape. `stats_table_week.json` and `stats_table_project.json` were still absent.
- `app/bin/build`, `SALAH_UI_SHOT_DIR=/tmp/salah-c4-shots app/bin/test`, and `git diff --check` passed. Qt reported 30 passed and 24 socket-dependent skips. Reviewed `prompt-pause-preview.png`. The socket-enabled `trackingUiScreens` test also captures `prompt-pause.png` from the Today running card.

CHECKPOINT 1: Add Today's optional pause panel with worked-time correction and Save & Done (`app/src/backend.cpp`, `app/src/backend.h`, `app/src/client/coreclient.cpp`, `app/src/client/coreclient.h`, `app/src/qml/Main.qml`, `app/src/qml/overlays/SessionPrompt.qml`, `app/src/qml/today/ItemCard.qml`, `app/src/qml/today/RunningCard.qml`, `app/src/qml/today/SessionRow.qml`, `app/tests/salah_app_tests.cpp`, `docs/work/tracking-ui/c4-progress.md`)

## Slice 2

- Tasks 1b-1d and 2-3. Running card: Distractions N with -/+ (`session.bump`) and Add note (`session.note`, timestamped entries listed on the card). The count is a field in the pause panel and session editor. Item editor has Asks chips (all on by default, Reset to project, `metric_ids` sent only when edited) and every panel asks the row's `metrics`. Expanded session rows list notes as local HH:MM + text. Statistics has separate Tags, Projects (narrowed by tag) and Tasks (narrowed by project) lists, a Custom range (from/to), and every chart has a Chart/Table toggle.
- `app/bin/build`, `app/bin/test` (56 passed, 0 skipped, sockets run) and `git diff --check` pass. Screenshots in /tmp/claude-1000/c4-shots: running-card, prompt-pause, item-editor-asks, session-notes, stats-filters, stats-chart-table.

CHECKPOINT 3: Add running distractions and notes, item Asks chips, timestamped session notes, stats Tags/Projects/Tasks filters and Chart/Table toggle (`app/src/backend.cpp`, `app/src/backend.h`, `app/src/client/coreclient.cpp`, `app/src/client/coreclient.h`, `app/src/models/daytimelinemodel.cpp`, `app/src/models/daytimelinemodel.h`, `app/src/models/viewstate.cpp`, `app/src/models/viewstate.h`, `app/src/resources.qrc`, `app/src/qml/Main.qml`, `app/src/qml/overlays/EntryFields.qml`, `app/src/qml/overlays/EntryForm.qml`, `app/src/qml/overlays/SessionPrompt.qml`, `app/src/qml/pages/TodayPage.qml`, `app/src/qml/pages/StatisticsPage.qml`, `app/src/qml/today/ItemCard.qml`, `app/src/qml/today/RunningCard.qml`, `app/src/qml/today/SessionRow.qml`, `app/src/qml/today/Timeline.qml`, `app/src/qml/statistics/ChartModeToggle.qml`, `app/src/qml/statistics/ChartTable.qml`, `app/src/qml/statistics/DayColumnChart.qml`, `app/src/qml/statistics/LineChart.qml`, `app/tests/salah_app_tests.cpp`, `app/tests/tracking_fixture.h`, `docs/work/tracking-ui/c4-progress.md`)
