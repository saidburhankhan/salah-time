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

## Task 1 — goal chips

The panel header shows today's and this week's goals from `status.get.goals`
as compact chips below the hero, above the running card. `GoalLogic.js`
orders today's goals before the week's (server position order within each),
keeps the first three and reports the rest as "+N". Each chip shows the
label, `value / target` with a unit (`≤` for at_most goals), a thin progress
bar and a muted check when met. `GoalChips.qml` renders them and hides
entirely when there are no goals. A metric goal with no recorded value shows
"—" and an empty bar. Goals carry only ids, so labels come from the loaded
items, epics, projects and metric catalogue: item title, epic name, project
name, "All work", with " · Metric" appended for metric goals (an unresolved
scope shows an empty label). Week goals carry a small "Wk" prefix. All text
is `Text.PlainText` in muted theme tints; the bar "+" click target is
untouched. Strings are in `i18n/en.json` (`goal.*`).

Checks: all node tests passed, including 7 new ones in `goal_logic_test.js`
(fixture shape, empty or missing list, more than 3 goals, met and unmet,
at_most, null metric value, array-like input). `omarchy plugin validate`,
`jq empty` and `git diff --check` passed. qmllint reports no syntax errors
(only unresolved `qs.*` import noise). No offscreen render was run.

CHECKPOINT 1: Add panel header goal chips from status.get.goals — files: plugin/local.salah/GoalLogic.js, plugin/local.salah/GoalChips.qml, plugin/local.salah/BarWidget.qml, plugin/local.salah/i18n/en.json, plugin/local.salah/tests/goal_logic_test.js, docs/work/tracking-ui/c10-progress.md

Orchestrator follow-up (render review): an item goal whose task is not on today's plan rendered a
blank chip label. `scopeLabel` now prefers a server `scope_label` (queued for C8 as an additive
GoalView field) and otherwise falls back to "Task"/"Epic"/"Project". Render with a scratch daemon
built at bdc257e: "All work 25 / 60 min", "Task 25 / 20 min ✓", "Wk Quran 4 / 300 min", "+1".
8 goal checks pass.

## Task 2 — weekly review footer

C8's checkpoint 1b fixture confirms that the chips use the server's project,
epic and task labels, even when the local catalogues contain older names.
All-work goals still use the widget's localized label. The goal logic
already had the correct precedence, so only its tests changed.

The footer checks `review.get` for the previous Monday when the open panel's
status says it is Monday after Fajr in the configured location. Only a
matching reply with a null note makes the link eligible. The widget rereads
on review changes over the socket and polls in stdio fallback mode. A
request ID prevents older replies from restoring a stale link. The prompt
is inline only.

**Question for the orchestrator and C9.** Which page argument should the
widget pass to `salah open`? The existing CLI accepts no arguments for
`open`, and `salah-app` starts on Today without reading a page argument.
Can the CLI forward `--page review` to the app? The footer stays hidden
until the supported argv is set in `weeklyReviewCommand`.

All 138 Node checks, Omarchy plugin validation, QML syntax checks and
`git diff --check` passed. An offscreen run with a layer-shell stand-in
and a stub CLI exercised the stdio request, visible footer, stale reply,
saved note, review invalidation and fixed-argv click. The stand-in
preserved the real KeyboardPanel's Items-only content property. The
actual Wayland panel, socket transport and app launch need integration
verification once the backend and launcher are available.

CHECKPOINT 2: Add weekly review footer logic and verify server goal labels — files: plugin/local.salah/BarWidget.qml, plugin/local.salah/ReviewLogic.js, plugin/local.salah/SalahClient.qml, plugin/local.salah/i18n/en.json, plugin/local.salah/tests/goal_logic_test.js, plugin/local.salah/tests/review_logic_test.js, docs/work/tracking-ui/c10-progress.md
