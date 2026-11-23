# C6 widget progress

## Task 1 — creation sheet

The sheet uses the core’s `item.parse` result for its title, block, minutes and recurrence. Choosing a chip overrides the prefill. Tags filter active projects, and a task can be saved without a project. Tomorrow follows the planner’s tracking day. Both backlog actions send a one-time task without a date or block. The timeline opens at its current block in a scrollable viewport below the sheet. Enter retains quick add and Escape dismisses the draft.

All 89 Node checks passed (6 creation, 29 drag, 54 session), along with Omarchy plugin validation, QML syntax parsing and `git diff --check`. An offscreen QML interaction check with a mock transport exercised parsing, stale replies, both save buttons, a rejected save, duplicate-click protection, Enter and Escape. Socket integration with the phase 3 backend and full-popup visual review are pending with the orchestrator.

CHECKPOINT 1: Add the parsed widget creation sheet with project filters, scheduling and backlog actions (`plugin/local.salah/BarWidget.qml`, `plugin/local.salah/CreationChipRow.qml`, `plugin/local.salah/CreationLogic.js`, `plugin/local.salah/QuickAdd.qml`, `plugin/local.salah/SalahClient.qml`, `plugin/local.salah/i18n/en.json`, `plugin/local.salah/tests/creation_logic_test.js`, `docs/work/tracking-ui/c6-progress.md`)
