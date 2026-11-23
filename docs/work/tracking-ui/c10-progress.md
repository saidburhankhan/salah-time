# C10 widget progress

## Task tags

The creation sheet loads the full `tags.list` catalogue over the socket or
stdio RPC fallback. Choosing a project selects its default tags for the
draft. Changing projects starts with the new project's defaults. The tag
chips toggle independently of the project and epic pickers, including for
tasks without a project. Add and To backlog send the selected `tag_ids`,
including an empty list when all defaults are cleared. Enter retains the
quick-text submission path.

All 125 Node checks passed, including 14 for creation logic. Omarchy plugin
validation, QML syntax checks and `git diff --check` passed. An offscreen
Quickshell render with a stub transport showed the muted selected state.
The same run verified that toggling a standalone tag kept the project
selected and sent the expected backlog payload. Socket integration is
pending orchestrator verification.

CHECKPOINT 0: Add task tag selection to the widget creation sheet — files: plugin/local.salah/BarWidget.qml, plugin/local.salah/CreationChipRow.qml, plugin/local.salah/CreationLogic.js, plugin/local.salah/QuickAdd.qml, plugin/local.salah/SalahClient.qml, plugin/local.salah/i18n/en.json, plugin/local.salah/tests/creation_logic_test.js, docs/work/tracking-ui/c10-progress.md
