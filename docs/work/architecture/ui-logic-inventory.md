# UI logic inventory: domain rules still computed in app/plugin

This is a static inventory of domain rules that `app/src/qml/**` and
`plugin/local.salah/*.qml` compute themselves. It is the deletion checklist for
migration step 6 ("Move domain projections to Rust"). The replacement column
names the protocol v1 field or method that now carries the authoritative value;
"presentation only" means the rule is allowed to stay in the client because it
is layout, not domain policy.

Line numbers refer to the `architecture` branch at the time of this inventory.

## Desktop app (`app/src/qml`)

| File:line | Rule computed locally | Replaced by |
|---|---|---|
| `Main.qml:82-88` | `weekdayText` maps repeat/weekday flags to "Daily", "Mon–Fri", "Mon, Wed, Fri". | `schedule_text` on every item row. |
| `Main.qml:90-93` | `detail` joins measured progress and the repeat label. | `detail_text` on every item row. |
| `Main.qml:95-99` | `runningMinutes` adds elapsed minutes since `timer_started_at` to `minutes`. | `timer.minutes_including_running` / `timer.started_at`. |
| `Main.qml:101-106` | `nextOccurrence` scans up to 14 days for the first weekly weekday. | `next_occurrence` on rows; `item.preview.next_occurrence` for drafts. |
| `Main.qml:107-111` | `occursOn` decides whether an item applies to the selected day. | `next_occurrence`, `day.get` rows, `item.preview`. |
| `Main.qml:113-118` | `maybeToast` re-checks occurrence and recomputes the next date for the toast. | `item.preview.next_occurrence`. |
| `Main.qml:133-137` | `calendarDate` derives a Monday-first six-week grid from a date. | Presentation only (calendar layout). |
| `Main.qml:139-144` | `monthStep` clamps a date when stepping months. | Presentation only (navigation). |
| `Main.qml:146-157` | `statsBounds` normalises week/month/custom ranges and swaps reversed dates. | Client supplies `from`/`to`; range selection remains presentation. |
| `Main.qml:158-168` | `stepStats` derives the next range and `statsRangeLabel` formats it. | Presentation only. |
| `Main.qml:171-176` | `tooltipText`/`tooltipTextAt` build chart hover text from done/planned/minutes/count/prayers. | `stats.daily[].tooltip`. |
| `Main.qml:178-190` | `selectedDaily` rebuilds per-item daily rows from `item.series`; `shownDaily`/`newestDaily` filter and reverse. | `stats.daily` plus `stats.items[].series`; filtering/reversing remains presentation. |
| `Main.qml:191-196` | `shownTotals` and `maxDaily` re-sum and re-scale selected rows. | `stats.totals` for the whole range; per-selection totals remain presentation. |
| `Main.qml:197-207` | `niceAxis` chooses axis steps and a rounded maximum. | `stats.time_ticks`. |
| `Main.qml:208-213` | `axisLabel` formats minute ticks as `30m`, `1h`, `1h30`. | `stats.time_ticks[].label`. |
| `overlays/EntryForm.qml:180-193` | Preview labels "Next occurrence" from the selected day. | `item.preview.next_occurrence`. |
| `overlays/EntryForm.qml:196-205` | "Occurrences this month" as `30` for daily, `days.length * 4` for weekly, `1` for once. | `item.preview.occurrences_this_month`. |
| `overlays/EntryForm.qml:207-214` | "Schedule" preview repeats the weekday mapping. | `item.preview.schedule_text`. |
| `overlays/EntryForm.qml:57-68` | `selectRepeat` supplies Mon–Fri for "weekdays" and the selected weekday for "weekly". | Client form default; `item.preview` validates the resulting draft. |
| `today/Timeline.qml:20-47` | `desiredTimelineRows` joins items to blocks, inserts add/marker rows, and orders them. | `day.get.blocks[].items` with `position` and display fields; row assembly stays presentation. |
| `today/Timeline.qml:71-99` | Drag preview computes a visual gap and renumbers the optimistic list. | Presentation only; send `after_id`/`before_id` on commit. |
| `today/Timeline.qml:100-120` | `finishDrag` turns a row index into `(block, index)` for `item.move`. | `item.move` with `after_id`/`before_id`; no `item.move_target` needed. |
| `statistics/DayColumnChart.qml:14-17,55-79` | Chooses chart axis lines, step, maximum, and labels. | `stats.time_ticks`. |
| `statistics/DayColumnChart.qml:146` | Builds per-day hover text. | `stats.daily[].tooltip`. |
| `statistics/PrayerDayChart.qml:106-114` | Builds per-day hover text. | `stats.daily[].tooltip`. |
| `statistics/DayTable.qml:8` | Reverses daily rows for the table. | Presentation only (`stats.daily` is oldest first). |
| `pages/RoutinesPage.qml:45` | Formats a routine's repeat/weekdays via `weekdayText`. | `stats.items[].schedule_text`. |
| `pages/RoutinesPage.qml:51-54` | For a routine with no planned days, computes "Starts …" from `nextOccurrence(modelData, todayDay)`. | `stats.items[].next_occurrence`. |

## Bar widget (`plugin/local.salah`)

| File:line | Rule computed locally | Replaced by |
|---|---|---|
| `BarWidget.qml:19-20` | Keeps `remainingSeconds` from `status.next.remaining_seconds`. | `status.next.at` / `clock.boundary.next.at`; count from one server instant. |
| `BarWidget.qml:50-58` | `runningTimer` and timer minutes add elapsed time from `started_at` to two local clocks. | Row `timer.running`, `timer.started_at`, `timer.minutes_including_running`. |
| `BarWidget.qml:70-82` | `formatRemaining` converts remaining seconds into countdown text. | Presentation only; derive seconds from `next.at`. |
| `BarWidget.qml:155-160` | `itemsForBlock` filters items by block and sorts by numeric position. | `day.get.blocks[].items` / `status.planner` rows already grouped and ordered by `position`. |
| `BarWidget.qml:164-169` | `boundary(id)` scans `status.boundaries` to find a prayer/sleep row. | `day.now`/`status.boundaries` remain view data; block/next identity comes from `clock.boundary`. |
| `BarWidget.qml:171-220` | Optimistic done/timer/count/minute transitions and capped local counters. | Canonical `changed.items` rows from `entry.*`/`timer.*`; display fields above. |
| `BarWidget.qml:239-245` | `itemDetail` joins minutes/target and repeat labels. | `detail_text` / `schedule_text`. |
| `BarWidget.qml:279-293` | `acceptReport` sets a local `boundaryTimer` for exactly `remaining_seconds`. | `clock.boundary` push plus `next.at`; no local boundary timer. |
| `BarWidget.qml:590-646` | `desiredRows` builds six blocks, joins items, sorts them, inserts Sunrise/Sleep markers, and derives `current`. | `day.get.blocks`, `day.now`, `status.next`; row assembly stays presentation. |
| `BarWidget.qml:654-676` | `syncTimeline` diffs and reorders the display model to match locally built rows. | Presentation only once rows come from `day.get`. |
| `BarWidget.qml:727-753` | `showNextOccurrence` parses repeat/weekdays and scans up to seven days for the next occurrence hint. | `item.preview.next_occurrence` / row `next_occurrence`. |
| `BarWidget.qml:768-779` | `addPendingItem` picks a fallback block and inserts the pending row at a locally computed add-row index. | Optional pending presentation; canonical block comes from `item.add` / `status.planner.planning_block`. |
| `BarWidget.qml:782-810` | `beginDrag`/`moveDrag` snapshot and renumber rows to show a visual gap. | Presentation only; send `after_id`/`before_id`. |
| `BarWidget.qml:825-846` | `finishDrag` walks rows to derive `(block, blockIndex)` and sends a numeric index. | `item.move` with `after_id`/`before_id`; no `item.move_target` needed. |
| `SalahClient.qml:377-386` | `moveItem` forwards a locally computed block index (and a CLI `--index` fallback). | `item.move` with `after_id`/`before_id`. |
| `TimelineRow.qml:70-73` | Recomputes running minutes from `timer_started_at` and `Date.now()`. | Row `timer.minutes_including_running`; countdown only for display. |
| `TimelineRow.qml:59-62` | Disables inline title editing for repeating items. | Presentation/UX rule; `item.edit` remains authoritative. |
| `RowActions.qml:110-134` | Shows delete/edit only for one-time items and timer controls for measured items. | Presentation; actions still validated by `item.edit`/`item.remove`. |

## Not replaced in this step

These remain client responsibilities because they are interaction, formatting,
or layout, not domain projection: calendar grid layout, chart geometry/future
bar dimming, range stepping controls, focus/selection, optimistic pending rows,
transient error banners, and locale-specific date/number rendering.
