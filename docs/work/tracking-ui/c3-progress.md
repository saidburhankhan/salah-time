# C3 widget progress

Both Pause controls open the same session editor. Save and Save & Done submit one stop for the displayed session. Untouched Worked minutes and live metric counts stay under server control. Item panels use the effective row metrics, including an empty list. Older reports that omit the field retain their previous Done and Log prompts.

The bar plus has its own registered click target. Both buttons count a distraction and pulse on acknowledgement. Right-click also opens a one-line note field anchored to the bar. The running card offers the same editor inline and can adjust the count in either direction. Expanded sessions show their main note and timestamped entries in local time.

The CLI fallback passes the supplied stop flags, including `--done` and `--minutes`. Bumps and timestamped notes use `salah rpc` over stdin in fallback mode.

All 54 session checks and 29 drag checks passed. Omarchy plugin validation, QML syntax parsing, and `git diff --check` passed. Screen rendering and socket integration are reserved for the orchestrator.

Files under `plugin/local.salah/` are `BarWidget.qml`, `MetricField.qml`, `RowActions.qml`, `RunningCard.qml`, `SalahClient.qml`, `SessionLogic.js`, `SessionNoteField.qml`, `SessionNotePopover.qml`, `SessionPanel.qml`, `i18n/en.json`, and `tests/session_logic_test.js`. The checkpoint record is `docs/work/tracking-ui/c3-progress.md`.

CHECKPOINT P2: Add widget pause saves, live distraction controls, and timestamped session notes
