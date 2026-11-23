# TR progress

## Checkpoint 1 verification

Trends follows the shared Statistics filter and reads `stats.series` over RPC or the CLI fallback. The first eight series are visible in server order. Turning one off frees its colour for another series, including Other. A constant series in Relative view sits at the midpoint.

`app/bin/build`, `SALAH_UI_SHOT_DIR=/tmp/salah-tr-1 app/bin/test` and `git diff --check` passed. Qt reported 32 passed, 0 failed and 29 skipped because the sandbox cannot create Unix sockets. The socket-independent screenshot test exercised the CLI request path and checked the 31-day threshold, the eight-line cap, gaps, zero minutes, the table and the raw-value tooltip. Opened every PNG in `/tmp/salah-tr-1` and reviewed the 28-day Focus-by-task, mixed Split and mixed Relative charts and the existing pause preview.

### Tests skipped for Unix sockets

- `socketHandshakeSubscribesAndCorrelatesRequests`
- `socketRequestTimesOut`
- `socketReconnectsAfterDrop`
- `socketConflictReloadsAndShowsServerMessage`
- `changedNotificationReloadsOncePerRevision`
- `socketMutationAppliesCanonicalRows`
- `refusingSocketFallsBackToCliImmediately`
- `socketMoveSendsNeighbourAnchors`
- `coreProjectionTextDrivesApp`
- `timelineDragIntoEmptyBlockShowsGapAndSendsAnchors`
- `timelineDragWithinBlockSendsAfterAndBeforeAnchors`
- `timelineDragEscapeCancelsAndRestoresOrder`
- `timelineDragFailedMoveRestoresServerOrder`
- `timelineDragClampsAboveAndBelowTheBlocks`
- `timelineOrdersItemsByBlockAndPosition`
- `singleRowMutationChangesOnlyThatRow`
- `settingsListsAreSeparateAndEntitySavesSendFullFields`
- `itemEditorPicksAndCreatesProjects`
- `tickUntickNeverLogsASecondTick`
- `sessionWritesSendMinutesValuesAndNotes`
- `pausePanelSendsOnlyChanges`
- `runningSessionComesFromStatus`
- `statsFilterAndRangeReachTheDaemon`
- `bumpNoteAndAsksReachTheDaemon`
- `statsTaskAndCustomRangeFiltersReachTheDaemon`
- `statsTableSessionsListAndMetricFiltersReachTheDaemon`
- `canonicalTableAndListFixturesReachTheModels`
- `filterBarTableAndTabsFollowTheUser`
- `trackingUiScreens`

CHECKPOINT 1: Add filtered day and week Trends charts with split and relative views (`app/src/backend.h`, `app/src/backend_reports.cpp`, `app/src/client/coreclient.cpp`, `app/src/client/coreclient.h`, `app/src/qml/Main.qml`, `app/src/qml/pages/StatisticsPage.qml`, `app/src/qml/statistics/ChartTable.qml`, `app/src/qml/statistics/ChartsView.qml`, `app/src/qml/statistics/TrendsChart.qml`, `app/src/qml/statistics/TrendsPlot.qml`, `app/src/resources.qrc`, `app/tests/salah_app_tests.cpp`, `app/tests/tracking_fixture.h`, `docs/work/tracking-ui/tr-progress.md`)

## Checkpoint 2 verification

The Today running card shows the server's Sleep clamp in tabular minutes and seconds and adds the muted line “Ran past Sleep: stop it to save up to N min”. A stale response without a cap shows a dash and asks the user to stop the session. The screenshot test covers the card through daemon and CLI fixtures. The CLI test verifies that a fresh timer advances and a stale timer stays at 120:00 as the clock advances.

`app/bin/build`, `SALAH_UI_SHOT_DIR=/tmp/salah-tr-2 app/bin/test` and `git diff --check` passed. Qt reported 33 passed, 0 failed and the same 29 Unix socket skips listed above. Opened all five PNGs in `/tmp/salah-tr-2`. The stale running card's label fits on one line and its timer reads 120:00.

CHECKPOINT 2: Freeze the stale running card at the Sleep clamp and explain how to save it (`app/src/qml/Main.qml`, `app/src/qml/today/RunningCard.qml`, `app/tests/salah_app_tests.cpp`, `app/tests/tracking_fixture.h`, `docs/work/tracking-ui/tr-progress.md`)
