# Plan: redesign

## Outcome

The Salah desktop app and bar widget are redesigned around the prayer
timeline (layout reference: user-supplied screenshots of a prayer-centred
planner; our own identity, not a copy). Routines are as easy to create as
todos. Prayers can be marked as prayed.

## Decisions (user, 2026-11-22)

- Prayer circles mark the prayer as prayed (private prayer tracking).
- Repeat: once, daily, weekdays (Mon–Fri), weekly with chosen days.
- Widget popup: compact timeline of the day with quick add.
- App colours and corner rounding follow the Omarchy theme; layout and
  typography follow the reference (mono times, soft cards, white primary).

## App structure

- Sidebar: Today, Routines, Statistics, (space for Projects), Settings;
  bottom: location.
- Today: header "Today · <weekday> <Hijri date>   <Gregorian date>", quick
  add button. Timeline: each prayer row (circle to mark prayed, name, time,
  NOW badge on the current block) followed by its items and "+ add".
  Items drag-and-drop within and between blocks. Right column: Now card
  (next boundary countdown) and a month calendar that selects the day.
- Quick add (Ctrl+K / "+ add"): one line, natural language
  ("read 20m after isha daily"), Enter logs it, "Edit details" opens the
  entry form with the parsed draft.
- Entry form: title; When (prayer chips); measure + amount (tick /
  minutes / count); Repeat (once, daily, weekdays, weekly + day picker);
  preview (next occurrence, occurrences this month).
- Routines: repeating items with a 14-day chart; edit and archive.
- Statistics: per-item totals and prayer completion over 7/30 days.
- Settings: sleep time, location, method (via `salah config`).

## Non-goals

Categories/tags, projects, fixed clock times, monthly repeat, invites,
Hours view, prayer ring visualisation (may follow).

## Slices

- [x] C1 — core: weekly repeat, item position, prayer marks, quick-add
      parser, day plan, stats, export v2 (lead; `src/`)
- [x] C2 — CLI for the contract (lead or worker; `src/main.rs`, tests)
- [x] A1 — app redesign (Codex; `app/**`)
- [x] W1 — widget redesign (Codex; `plugin/**`)
- [ ] I1 — integration against the real CLI, install, live check

## Acceptance checks

- [ ] A routine (daily / weekdays / chosen days) can be created from the app
      and from the widget's quick add.
- [ ] Items can be dragged within and between blocks; order persists.
- [ ] Prayers can be marked and unmarked; marks appear in statistics.
- [ ] "read 20m after isha daily" creates a daily 20-minute Isha routine.
- [ ] Export v1 files still import; export v2 round-trips prayer marks.
- [ ] App never blocks the UI on a CLI call.
- [ ] `./scripts/test`, strict Clippy, app tests pass; live check done.
