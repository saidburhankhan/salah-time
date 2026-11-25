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

## Feedback items 3 and 4 — resource drafts and link validation

In an expanded task, a bare domain with an optional path saves as HTTPS.
An invalid link shows the web-address hint under the field while Save is
disabled. Explicit HTTP(S) links keep their scheme.

The creation sheet has a collapsible Notes & links section for notes, links
and reminders. A reminder date and time use the desktop's local time zone.
Add and To backlog include a valid draft even if Attach was skipped. An
invalid draft remains editable and blocks submission with its inline hint.
Enter still quick-adds a bare task line and uses the sheet when resources
are present.

After `item.add` confirms the task ID, the sheet attempts each resource
in order. A write failure keeps the task and shows an inline error with a
retry action. Retry sends only failed writes with their original op IDs.
Retrying an unchanged task submission also reuses its op ID.

All 144 Node checks, Omarchy plugin validation, QML syntax checks, JSON
parsing and `git diff --check` passed. Offscreen Quickshell with an
Items-only panel stand-in and stub stdio RPC verified the inline hint,
HTTPS request, note and local-time reminder payloads, scheduled and
backlog task creation, and an identical link retry with no second task
write. Wayland and Unix-socket verification remain with the orchestrator.

CHECKPOINT 3: Normalize widget link drafts and attach retryable resources to new tasks — files: plugin/local.salah/BarWidget.qml, plugin/local.salah/CreationLogic.js, plugin/local.salah/CreationResources.qml, plugin/local.salah/ItemResources.qml, plugin/local.salah/QuickAdd.qml, plugin/local.salah/ResourceLogic.js, plugin/local.salah/i18n/en.json, plugin/local.salah/tests/creation_logic_test.js, plugin/local.salah/tests/resource_logic_test.js, docs/work/tracking-ui/c10-progress.md

## R4 — replay unconfirmed task creation and preserve drafts during title edits

The creation sheet snapshots the operation ID and payload before sending
`item.add`. Retry replays that snapshot across Sleep or catalogue refreshes.
Attachments use the item ID from the confirmed response. A definite
rejection reopens editing, including in stdio mode where the client now
passes through the server's structured error data.

Clearing the title stops parsing and leaves the sheet open with its tag
choices, queued attachments and the resource being edited. Dismissal or
successful completion resets the draft. Enter on a bare line still uses
quick add, while explicit sheet choices use the parsed submission.

All 147 Node checks, Omarchy plugin validation, QML syntax checks, JSON
parsing and `git diff --check` passed. An offscreen run with the Items-only
panel stand-in and stub stdio RPC kept a chosen tag, queued note and
unfinished link through title replacement. After a simulated lost reply and
tracking-day rollover, the recorded retry matched the original payload
and operation ID, and both resources targeted the original item. A matched
validation error unlocked editing. Live Wayland and socket verification
remain with the orchestrator.

CHECKPOINT 4: Replay unconfirmed widget task creation and preserve drafts through title edits — files: plugin/local.salah/CreationLogic.js, plugin/local.salah/QuickAdd.qml, plugin/local.salah/SalahClient.qml, plugin/local.salah/i18n/en.json, plugin/local.salah/tests/creation_logic_test.js, docs/work/tracking-ui/c10-progress.md

## R5 — goal progress overflow

Goal chips show “Total too large” beside the target when progress reports
`sum_overflow`. A period with no samples still shows a dash. The Node
regression checks both cases and confirms that overflow has no met check.
All 148 Node checks, Omarchy plugin validation, QML syntax checks, JSON
parsing and `git diff --check` passed.

CHECKPOINT 5: Show goal sum overflow distinctly in widget chips — files: plugin/local.salah/GoalChips.qml, plugin/local.salah/GoalLogic.js, plugin/local.salah/i18n/en.json, plugin/local.salah/tests/goal_logic_test.js, docs/work/tracking-ui/c10-progress.md

## Ended routines — widget verification

The timeline groups `status.get.planner.items` by prayer block. The CLI
fallback reads the same projection through `salah status --json`. The widget
has no occurrence filter to update, and the creation sheet and item row
actions construct explicit payloads that omit `end_day`. No plugin changes or
new Node test were needed.

With migration 006 in a scratch database, `day.get`, `status.get` and
`salah status --json` agreed before and after Sleep. The routine appeared on
its inclusive end day and disappeared from the next day's plan. The
title-only edit preserved `end_day`, and the historical day retained the
routine's 12 recorded minutes. An offscreen widget run replayed those
status snapshots through the stdio path and confirmed that the timeline
removed the ended routine and kept the active one. The panel used an
Items-only stand-in for Wayland.

All 51 stdio RPC tests, including the updated fixtures and routine-end
regression, and all 148 Node checks passed. Omarchy plugin validation and
`git diff --check` passed.

CHECKPOINT 6: Verify ended routines follow the backend plan in the widget — files: docs/work/tracking-ui/c10-progress.md

## Archived projects — paused rows, goals and creation choices

The timeline follows the backend's planner. If a refreshed plan removes
a task during a drag, the widget cancels the drag and applies the new
plan immediately. Goal chips omit paused rows before sorting and counting.
The creation sheet requests active projects and the default unpaused
epic catalogue, filters both choices, and clears a selection when it
becomes unavailable. An open sheet refreshes these catalogues with
status snapshots in stdio fallback mode.

All 151 Node checks and C8's three project-pause RPC tests passed, including
the paused catalogue and status fixtures. Omarchy validation, QML syntax
checks and `git diff --check` passed.

The offscreen Quickshell harness used C8's real CLI and a scratch database.
Archiving while the creation sheet was open and a routine was being
dragged removed the routine, open task, scoped goal chips, project and
epic. The all-work goal and unrelated task remained. Resuming the project
restored the applicable timeline rows and choices. Both captures were
reviewed in `/tmp/salah-c10-paused.IJ1gdW/`. The harness used an Items-only
panel stand-in and stdio RPC.

CHECKPOINT 7: Keep paused project work out of the widget timeline, goal chips and creation choices — files: plugin/local.salah/BarWidget.qml, plugin/local.salah/CreationLogic.js, plugin/local.salah/GoalLogic.js, plugin/local.salah/QuickAdd.qml, plugin/local.salah/tests/creation_logic_test.js, plugin/local.salah/tests/goal_logic_test.js, docs/work/tracking-ui/c10-progress.md
