# C4 progress

## Slice 1

- All three Today pause controls open the optional session panel. Worked starts at the rounded elapsed time when the panel opens. The daemon validates duration overrides. Save & Done is available for item sessions and uses one `session.stop` write.
- Implemented against `plan.md`. `session_stop_done.json` and `session_stop_minutes.json` appeared in the backend worktree during verification and match the expected result shape. `stats_table_week.json` and `stats_table_project.json` were still absent.
- `app/bin/build`, `SALAH_UI_SHOT_DIR=/tmp/salah-c4-shots app/bin/test`, and `git diff --check` passed. Qt reported 30 passed and 24 socket-dependent skips. Reviewed `prompt-pause-preview.png`. The socket-enabled `trackingUiScreens` test also captures `prompt-pause.png` from the Today running card.

CHECKPOINT 1: Add Today's optional pause panel with worked-time correction and Save & Done (`app/src/backend.cpp`, `app/src/backend.h`, `app/src/client/coreclient.cpp`, `app/src/client/coreclient.h`, `app/src/qml/Main.qml`, `app/src/qml/overlays/SessionPrompt.qml`, `app/src/qml/today/ItemCard.qml`, `app/src/qml/today/RunningCard.qml`, `app/src/qml/today/SessionRow.qml`, `app/tests/salah_app_tests.cpp`, `docs/work/tracking-ui/c4-progress.md`)
