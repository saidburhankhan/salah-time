# C6 widget progress

## Task 1 — creation sheet

The sheet uses the core’s `item.parse` result for its title, block, minutes and recurrence. Choosing a chip overrides the prefill. Tags filter active projects, and a task can be saved without a project. Tomorrow follows the planner’s tracking day. Both backlog actions send a one-time task without a date or block. The timeline opens at its current block in a scrollable viewport below the sheet. Enter retains quick add and Escape dismisses the draft.

All 89 Node checks passed (6 creation, 29 drag, 54 session), along with Omarchy plugin validation, QML syntax parsing and `git diff --check`. An offscreen QML interaction check with a mock transport exercised parsing, stale replies, both save buttons, a rejected save, duplicate-click protection, Enter and Escape. Socket integration with the phase 3 backend and full-popup visual review are pending with the orchestrator.

CHECKPOINT 1: Add the parsed widget creation sheet with project filters, scheduling and backlog actions (`plugin/local.salah/BarWidget.qml`, `plugin/local.salah/CreationChipRow.qml`, `plugin/local.salah/CreationLogic.js`, `plugin/local.salah/QuickAdd.qml`, `plugin/local.salah/SalahClient.qml`, `plugin/local.salah/i18n/en.json`, `plugin/local.salah/tests/creation_logic_test.js`, `docs/work/tracking-ui/c6-progress.md`)

## Checkpoint 1c — pause cap and parsed date

Worked starts at the lower of rounded elapsed minutes and the server's `max_minutes`. Opening Pause refreshes status for a current cap. The correction chips clamp at one minute, and Save validates against the same capped value. The stale timer freezes at the reported Sleep cutoff and shows a muted line with the maximum duration that can be saved.

The creation sheet uses a parsed `start_day` or `day` to select When and set the date it sends. An explicit When chip overrides the prefill.

All 99 Node checks passed (8 creation, 29 drag, 62 session), along with Omarchy plugin validation, QML syntax parsing, translation JSON parsing and `git diff --check`. The cap tests use the `status_stale.json` fixture. Socket and visual review of this update remain with the orchestrator.

CHECKPOINT 1c: Cap widget pause minutes at Sleep and prefill When from the parsed day (`plugin/local.salah/BarWidget.qml`, `plugin/local.salah/CreationLogic.js`, `plugin/local.salah/QuickAdd.qml`, `plugin/local.salah/RunningCard.qml`, `plugin/local.salah/SessionLogic.js`, `plugin/local.salah/SessionPanel.qml`, `plugin/local.salah/i18n/en.json`, `plugin/local.salah/tests/creation_logic_test.js`, `plugin/local.salah/tests/session_logic_test.js`, `docs/work/tracking-ui/c6-progress.md`)

## Checkpoint 1b — epics

The creation sheet shows an optional Epic chip row after Project, offering only that project's active epics from `epics.list` ("No epic" default) and sending `epic_id` with `item.add` only alongside its project. "+ Log" shows the same chips when a project target is chosen (task targets carry their own epic) and `session.log` takes `epic_id`, so epic-only logging works. Day rows append the epic name in muted text to the detail line. Epics reload on `changed` scopes `epics` and `projects`, on reconnect and when the sheet or panels open; rows resync when the list changes. Chip choices reset when the project changes or the epic disappears.

All 16 creation, 29 drag and 62 session Node checks passed (4 new epic checks), plus Omarchy plugin validation, i18n JSON parsing and `git diff --check`. Not rendered here; the orchestrator reviews the screens.

CHECKPOINT 1b: Add optional epic chips to the widget creation sheet and Log panel and show epic names on day rows (`plugin/local.salah/BarWidget.qml`, `plugin/local.salah/CreationLogic.js`, `plugin/local.salah/QuickAdd.qml`, `plugin/local.salah/SalahClient.qml`, `plugin/local.salah/SessionPanel.qml`, `plugin/local.salah/TimelineRow.qml`, `plugin/local.salah/i18n/en.json`, `plugin/local.salah/tests/creation_logic_test.js`, `docs/work/tracking-ui/c6-progress.md`)

## Task 2a — due reminders

The bell shows the due count beside the prayer label. The popup starts with the oldest due reminders, including reminders attached to epics. The list shows up to three rows at a time and scrolls for more. Ticking a reminder sends `resource.edit` with `done: true`, disables that control until the reply, and removes the reminder on acknowledgement. The 30-second status refresh catches reminders that become due between prayer boundaries.

All 106 Node checks passed, including three covering due ordering and compatibility with older status reports. Omarchy plugin validation, QML syntax parsing, i18n JSON parsing, and `git diff --check` passed. An offscreen QML test with a stub host checked the list height, literal title rendering, and tick interaction. The full bar and popup render and socket integration remain with the orchestrator.

CHECKPOINT 2a: Show and complete due reminders in the widget (`plugin/local.salah/BarWidget.qml`, `plugin/local.salah/DueReminders.qml`, `plugin/local.salah/ResourceLogic.js`, `plugin/local.salah/i18n/en.json`, `plugin/local.salah/tests/resource_logic_test.js`, `docs/work/tracking-ui/c6-progress.md`)

## Task 2b — day-row resources

The paperclip badge counts notes, links and open reminders from the day row. Expanding the row loads that item's resources in position order, with a bounded scrolling area and quick-add fields for notes and links. Both link controls validate the URL before opening it externally. The reminder control completes the displayed version and shares the due list's pending state. Resource changes reload the opened list, and unchanged quick-add retries reuse their operation ID.

All 110 Node checks passed, including seven resource checks. Omarchy plugin validation, QML syntax checks, i18n JSON parsing, and `git diff --check` passed. A mocked offscreen interaction check covered URL validation and note and link saves, reminder dispatch, stale replies, and retry. An offscreen Quickshell harness with stubbed popup windows and CLI fixtures verified the paperclip count and loading the expanded row over stdio. Socket integration and visual review remain with the orchestrator.

CHECKPOINT 2b: Add resource counts, links, reminder completion and quick add to widget rows (`plugin/local.salah/BarWidget.qml`, `plugin/local.salah/ItemResources.qml`, `plugin/local.salah/ResourceLogic.js`, `plugin/local.salah/RowActions.qml`, `plugin/local.salah/SalahClient.qml`, `plugin/local.salah/TimelineRow.qml`, `plugin/local.salah/i18n/en.json`, `plugin/local.salah/tests/resource_logic_test.js`, `docs/work/tracking-ui/c6-progress.md`)
