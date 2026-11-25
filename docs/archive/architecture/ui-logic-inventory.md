# UI logic inventory: domain rules still computed in app/plugin

This is a static inventory of domain rules that `app/src/qml/**` and
`plugin/local.salah/*.qml` compute themselves. It is the deletion checklist for
migration step 6 ("Move domain projections to Rust"). The replacement column
names the protocol v1 field or method that now carries the authoritative value;
"presentation only" means the rule is allowed to stay in the client because it
is layout, not domain policy.

Line numbers refer to the `architecture` branch at the time of this inventory.

## Desktop app (`app/src/qml`)

| File:line | Rule computed locally | Status / replacement |
|---|---|---|
| `Main.qml:82-88` | `weekdayText` maps repeat/weekday flags to "Daily", "Mon–Fri", "Mon, Wed, Fri". | **removed** — schedule text comes from `schedule_text` on `stats.items[]` (`pages/RoutinesPage.qml:45`), `item.preview.schedule_text` (`overlays/EntryForm.qml:237`), and `detail_text` on day rows. |
| `Main.qml:90-93` | `detail` joins measured progress and the repeat label. | **removed** — item rows render `detail_text` directly (`today/ItemCard.qml:79`, `today/DragGhost.qml:48`). |
| `Main.qml:95-99` | `runningMinutes` adds elapsed minutes since `timer_started_at` to `minutes`. | **removed** — `timerMinutes`/`timerRunning` (`Main.qml:89-103`) only display `timer.minutes_including_running` and count forward from the projection instant for a running session; no rule recomputation. |
| `Main.qml:101-106` | `nextOccurrence` scans up to 14 days for the first weekly weekday. | **removed** — `stats.items[].next_occurrence` (`pages/RoutinesPage.qml:51-52`) and `item.preview.next_occurrence` (`overlays/EntryForm.qml:207`). |
| `Main.qml:107-111` | `occursOn` decides whether an item applies to the selected day. | **removed** — rows come from `day.get`; drafts use `item.preview` (`Backend::previewItem`, `overlays/EntryForm.qml:35-39`). |
| `Main.qml:113-118` | `maybeToast` re-checks occurrence and recomputes the next date for the toast. | **removed** — `maybeToast` (`Main.qml:105-108`) requests `item.preview`; the toast uses `preview.next_occurrence` (`Main.qml:195-201`). |
| `Main.qml:133-137` | `calendarDate` derives a Monday-first six-week grid from a date. | **kept (rendering)** — calendar grid layout; no domain value leaves the view. |
| `Main.qml:139-144` | `monthStep` clamps a date when stepping months. | **kept (rendering)** — month navigation/clamping of a chosen date. |
| `Main.qml:146-157` | `statsBounds` normalises week/month/custom ranges and swaps reversed dates. | **kept (rendering)** — range selection is presentation; the resulting `from`/`to` are sent to `stats.get`. |
| `Main.qml:158-168` | `stepStats` derives the next range and `statsRangeLabel` formats it. | **kept (rendering)** — range stepping controls and locale date formatting. |
| `Main.qml:171-176` | `tooltipText`/`tooltipTextAt` build chart hover text from done/planned/minutes/count/prayers. | **removed** — charts read `stats.daily[].tooltip` (`statistics/DayColumnChart.qml:161`, `statistics/PrayerDayChart.qml:114`); `selectedDaily` only attaches that core text to filtered chart rows. |
| `Main.qml:178-190` | `selectedDaily` rebuilds per-item daily rows from `item.series`; `shownDaily`/`newestDaily` filter and reverse. | **kept (rendering)** — protocol exposes `stats.daily` plus `stats.items[].series`; mapping series to chart rows, filtering to today, and reversing for the table are presentation. |
| `Main.qml:191-196` | `shownTotals` and `maxDaily` re-sum and re-scale selected rows. | **kept (rendering)** — `maxDaily` was deleted with `niceAxis`; `shownTotals` remains a per-selection table sum of already-rendered rows (`statistics/DayTable.qml:95`). Range totals still come from `stats.totals`. |
| `Main.qml:197-207` | `niceAxis` chooses axis steps and a rounded maximum. | **removed** — the time chart draws `stats.time_ticks` (`statistics/DayColumnChart.qml:15-33`); the tasks-done chart only scales its bars to the rendered data maximum. |
| `Main.qml:208-213` | `axisLabel` formats minute ticks as `30m`, `1h`, `1h30`. | **removed** — labels come from `stats.time_ticks[].label` (`statistics/DayColumnChart.qml` tick repeater). |
| `overlays/EntryForm.qml:180-193` | Preview labels "Next occurrence" from the selected day. | **removed** — label renders `item.preview.next_occurrence` (`overlays/EntryForm.qml:207-209`); `gregorianHeader` only formats the date. |
| `overlays/EntryForm.qml:196-205` | "Occurrences this month" as `30` for daily, `days.length * 4` for weekly, `1` for once. | **removed** — label renders `item.preview.occurrences_this_month` (`overlays/EntryForm.qml:222-224`). |
| `overlays/EntryForm.qml:207-214` | "Schedule" preview repeats the weekday mapping. | **removed** — label renders `item.preview.schedule_text` (`overlays/EntryForm.qml:237`). |
| `overlays/EntryForm.qml:57-68` | `selectRepeat` supplies Mon–Fri for "weekdays" and the selected weekday for "weekly". | **kept (rendering)** — client form default only; the resulting draft is validated and summarized by `item.preview` (`EntryForm.qml:35-52`). |
| `today/Timeline.qml:20-47` | `desiredTimelineRows` joins items to blocks, inserts add/marker rows, and orders them. | **kept (rendering)** — `day.get.blocks` already groups and orders rows by `position`; assembling headers, add rows, and Sunrise/Sleep markers is layout. |
| `today/Timeline.qml:71-99` | Drag preview computes a visual gap and renumbers the optimistic list. | **kept (rendering)** — drag gap geometry only; `finishDrag` sends neighbour ids. |
| `today/Timeline.qml:100-120` | `finishDrag` turns a row index into `(block, index)` for `item.move`. | **removed** — `finishDrag` sends `after_id`/`before_id` (`Timeline.qml:106-122`); `Backend::legacyMoveIndex` only supplies the legacy CLI `--index` fallback. |
| `statistics/DayColumnChart.qml:14-17,55-79` | Chooses chart axis lines, step, maximum, and labels. | **removed** — time axis lines and labels come from `stats.time_ticks` (`DayColumnChart.qml:15-33`); chart geometry remains presentation. |
| `statistics/DayColumnChart.qml:146` | Builds per-day hover text. | **removed** — hover text is `stats.daily[].tooltip` attached to chart rows (`DayColumnChart.qml:161`). |
| `statistics/PrayerDayChart.qml:106-114` | Builds per-day hover text. | **removed** — hover text is `stats.daily[].tooltip` (`PrayerDayChart.qml:114`). |
| `statistics/DayTable.qml:8` | Reverses daily rows for the table. | **kept (rendering)** — `stats.daily` is oldest first; the table renders newest first. |
| `pages/RoutinesPage.qml:45` | Formats a routine's repeat/weekdays via `weekdayText`. | **removed** — renders `stats.items[].schedule_text` (`RoutinesPage.qml:43-45`). |
| `pages/RoutinesPage.qml:51-54` | For a routine with no planned days, computes "Starts …" from `nextOccurrence(modelData, todayDay)`. | **removed** — renders `stats.items[].next_occurrence` (`RoutinesPage.qml:51-53`). |

## Bar widget (`plugin/local.salah`)

| File:line | Rule computed locally | Replaced by |
|---|---|---|
| `BarWidget.qml:19-20` | Keeps `remainingSeconds` from `status.next.remaining_seconds`. | **removed** — `BarWidget.qml:20-30` counts down from `next.at` (`countdownDeadline`, `BarWidget.qml:100-108`, and `acceptReport`, `:250-263`). The CLI-only fallback converts its snapshot once for display. |
| `BarWidget.qml:50-58` | `runningTimer` and timer minutes add elapsed time from `started_at` to two local clocks. | **removed** — `BarWidget.qml:53-72` reads row `timer.running`, `timer.started_at`, `timer.minutes_including_running`; only the display minute increment from that snapshot remains. |
| `BarWidget.qml:70-82` | `formatRemaining` converts remaining seconds into countdown text. | **kept (rendering)** — `BarWidget.qml:82-94` formats the countdown derived from `next.at`. |
| `BarWidget.qml:155-160` | `itemsForBlock` filters items by block and sorts by numeric position. | **removed** — `desiredRows` (`BarWidget.qml:537-602`) walks the core-ordered `planner.items`; grouping by `block` is row assembly, with no local position sort. |
| `BarWidget.qml:164-169` | `boundary(id)` scans `status.boundaries` to find a prayer/sleep row. | **kept (rendering)** — `BarWidget.qml:176-181` is a view-data lookup for marker times; block identity comes from `planner.block` / `status.current_block.start.id`. |
| `BarWidget.qml:171-220` | Optimistic done/timer/count/minute transitions and capped local counters. | **removed** — actions now only send intents (`BarWidget.qml:183-221`) and render canonical `changed.items` in `applyChanged` (`:273-310`); no local state transition or capped counters. |
| `BarWidget.qml:239-245` | `itemDetail` joins minutes/target and repeat labels. | **removed** — `TimelineRow.qml:296` renders `detailText` from row `detail_text`; `schedule_text` is carried on the row. |
| `BarWidget.qml:279-293` | `acceptReport` sets a local `boundaryTimer` for exactly `remaining_seconds`. | **removed** — `acceptReport` (`BarWidget.qml:250-263`) only takes the `next.at` snapshot; `SalahClient.qml:167-171,229` subscribes to `clock.boundary` and refetches status. No local boundary timer. |
| `BarWidget.qml:590-646` | `desiredRows` builds six blocks, joins items, sorts them, inserts Sunrise/Sleep markers, and derives `current`. | **removed** — `desiredRows` (`BarWidget.qml:537-602`) assembles rows from ordered `planner.items`, core `planner.block`, `prayers[].passed`, and `prayer_marks`; Sunrise/Sleep times remain boundary view data. |
| `BarWidget.qml:654-676` | `syncTimeline` diffs and reorders the display model to match locally built rows. | **kept (rendering)** — `BarWidget.qml:610-634` diffs the display model to the core-backed row list. |
| `BarWidget.qml:727-753` | `showNextOccurrence` parses repeat/weekdays and scans up to seven days for the next occurrence hint. | **removed** — `showNextOccurrence` (`BarWidget.qml:671-681`) uses row `next_occurrence`; only parses that date for the weekday label. |
| `BarWidget.qml:768-779` | `addPendingItem` picks a fallback block and inserts the pending row at a locally computed add-row index. | **removed** — `addPendingItem` (`BarWidget.qml:683-696`) takes the canonical block from `planner.planning_block`; inserting the pending row at the add row is presentation. |
| `BarWidget.qml:782-810` | `beginDrag`/`moveDrag` snapshot and renumber rows to show a visual gap. | **kept (rendering)** — `BarWidget.qml:704-736` keeps only drag gap geometry; commit sends `after_id`/`before_id`. |
| `BarWidget.qml:825-846` | `finishDrag` walks rows to derive `(block, blockIndex)` and sends a numeric index. | **removed** — `finishDrag` (`BarWidget.qml:758-784`) sends `DragLogic.destination()` `after_id`/`before_id`; the CLI fallback keeps an index only because its command has no anchors. |
| `SalahClient.qml:377-386` | `moveItem` forwards a locally computed block index (and a CLI `--index` fallback). | **removed** — `SalahClient.qml:385-398` sends `item.move` with `after_id`/`before_id` over RPC (one anchor, else `index` for an empty block). |
| `TimelineRow.qml:70-73` | Recomputes running minutes from `timer_started_at` and `Date.now()`. | **removed** — `TimelineRow.qml:74-80` starts from row `timer.minutes_including_running`; only the display minute increment from the snapshot remains. |
| `TimelineRow.qml:59-62` | Disables inline title editing for repeating items. | **kept (rendering)** — `TimelineRow.qml:65-72` is a presentation/UX guard; `item.edit` remains authoritative. |
| `RowActions.qml:110-134` | Shows delete/edit only for one-time items and timer controls for measured items. | **kept (rendering)** — `RowActions.qml:110-134` is visibility presentation; actions are validated by `item.edit`/`item.remove`. |

## Not replaced in this step

These remain client responsibilities because they are interaction, formatting,
or layout, not domain projection: calendar grid layout, chart geometry/future
bar dimming, range stepping controls, focus/selection, optimistic pending rows,
transient error banners, and locale-specific date/number rendering.
