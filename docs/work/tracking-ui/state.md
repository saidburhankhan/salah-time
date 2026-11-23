# State: tracking-ui

## Now

Installed 2026-11-23 06:49 (875edf0); backend follow-ups 47fb1d1, b656d94 committed after.
- App (UI-A, Claude): Settings 9fae1d6, Today 49aad8d, Stats 1b51277, muted selected/primary 154b254.
- Widget (UI-W, Claude): 6907066, rework for play/pause and done ticks + dark surfaces 9bcc596.
- Backend (C1, Codex gpt-6.2-astra high): review fixes 8b4cb16, optional metrics and done ticks
  875edf0, stale timers 47fb1d1, catch-up bound b656d94, contract-v1 note (this commit).

## Verification

140 Rust tests incl. daemon sockets, app 52/52, widget node checks 29 + 35, clippy, fmt,
plugin validate. Scratch end-to-end: start → pause (not done) → tick while running (stops timer,
tick with focus, done) → untick (not done, minutes kept). Live install ok, no journal warnings.

## Next

- 11:27 timer `salah-phase2`: Codex C2 (backend) + C3 (widget). After C2's first checkpoint: dispatch C4
  (app). After C2 is fully committed: dispatch C5 (phase 3 backend); after C5's first checkpoint: C6
  (widget) and C7 (app), each after the previous worker on that directory is committed.
  If Codex hits its 5h limit, continue the same brief with a Claude subagent.
- Review gates (data-model.md): C2 checkpoint 0 (migration chain + 003) and C5 checkpoint 0
  (004) each get an independent Claude reviewer before more work builds on them; migrate a copy of
  the live DB in scratch and compare row counts before every install.

- Model tiers (user, 2026-11-23): hard parts on `MODEL=gpt-6.2-astra EFFORT=high`, normal work on
  the dispatcher default `gpt-6.2-sol` high. The model is chosen per dispatch/resume, so a worker
  switches tier at its checkpoint.
  - Astra: C2 task 0 (migration chain + 003), C2 3 (`stats.table`) and 3b (`sessions.list`);
    C5 task 0 (004 items rebuild) and the backlog/epic core rules; C4 general table + Notes page;
    C6 widget creation sheet; any task that failed review once on Sol.
  - Sol: C2 1, 2, 2b, 2c, 2d, 4 (CLI), 5 (fixtures); C3 widget tasks; C4 pause panel,
    filters, Chart/Table toggle; C5 resources/CLI/fixtures; C6 rest; C7 app pages; docs.
  - Independent migration reviews stay with a Claude reviewer.

- 12:07: Codex 5h window at 95% (resets 16:27; weekly 76%, resets Sun 29 Nov 14:37). Codex C2 and
  C4 stopped mid-task; Claude subagents inherited the uncommitted work: C2 tasks 3/3b on Opus,
  C4 tasks 1b-3 on Sonnet. Same checkpoint protocol (no commits; orchestrator verifies and commits).
- Codex quota plan (user, 12:20): the subscription renews 25 Nov; the user has two limit resets
  and wants every bit used. Measured: one 5h window = about 31% of the weekly; three Astra workers
  burned a window in ~40 min, so quota (not wall clock) is the limit and Sol stretches it.
  Weekly 76% now, so the 16:27 window (~24% left) ends on the weekly cap; then the user applies
  reset 1, later reset 2 (a reset clears weekly + 5h; use one ONLY at a 100% weekly cap, never for
  a 5h cap). Keep the Codex queue full (disjoint dirs: crates / app / plugin / reviews):
  1. 16:27: C5 task 0 = migration 004 (Astra, crates; Claude review gate); C6 task 1 = widget
     creation sheet (Astra, plugin; user asked for it; Backlog button goes live when C5 task 1 lands);
     C4 tasks 4-5 were done by Claude (0b1cf3e). Small Sol fixes in the same window: widget Worked
     cap at `max_minutes` + stale display (plugin, after C6 task 1), app running card for a stale
     timer (app) = tr-brief.md task 2. TR task 1 = app Trends line chart (Astra, app; needs
     stats.series, being built by Claude now).
  2. After reset 1: C5 rest (epics first: user priority; then backlog; rules Astra; resources, CLI, fixtures Sol), C6 widget
     phase 3 (creation sheet Astra, rest Sol), C7 app phase 3 (Sol).
  3. Phase 4 (user chose insights, goals, weekly review; plan.md "Phase 4", data-model.md "005"):
     C8 backend (005 Astra + review gate), C9 app (Insights Astra), C10 widget (Sol).
  4. After reset 2: Astra reviews of phases 3-4, fix rounds, U5 polish.
  Expected timeline (one 5h window ≈ 31% weekly, so a fresh weekly ≈ 3.2 windows):
  - 16:27 window: weekly 76→100% around 17:00 → user applies reset 1 (fresh weekly + 5h).
  - Windows ~17:00, ~22:00, ~03:00 (24th), then ~0.2 of the ~08:00 window → weekly 100% ~08:15
    on the 24th → reset 2.
  - Windows ~08:15, ~13:15, ~18:15, then ~0.2 of ~23:15 → weekly 100% ~23:30 on the 24th, before
    the 25 Nov renewal. Requires every window started on time (overnight too: this session must
    stay open) and fully used. Recalibrate the 31% after each window.
  22:12 window (weekly 16%): C5 --resume Astra = r2-findings fixes; C6 --resume Sol = bell/due,
  resources in rows, then plugin r3 findings (CHECKPOINT R3w); C7 --resume Astra = all app/ r3
  findings (CHECKPOINT R3a; task drawer after, on Sol). session.log ticks already stop the
  matching timer server-side, so the R3 "Done = two writes" fix is frontend-only. C8 when C5 commits.
  Tags on tasks (user, ~18:00; plan.md addendum): C8 0 (item_tags in 005) + 0c rules, then C9 0
  (app) and C10 0 (widget) before their phase 4 tasks.
  Only useful work goes in the queue (user features, real bugs, data-guarding reviews), never
  filler; when Salah's queue is truly empty, ask the user which other project gets Codex.
  When Codex hits a cap: Claude subagents inherit (Opus hard, Sonnet normal) and the orchestrator
  tells the user so they can apply a reset.
- 12:45 installed phase 2 (backend through 8c7adfa, widget 029912a, app 0b1cf3e). The live DB was
  migrated to v3 at 12:43 by `scripts/test` (it overrode only HOME; the session exports XDG_*),
  checked lossless against bak-v1 (only 003's additions), integrity ok; script fixed in 461786e.
  Pending small fixes: widget Worked cap at `max_minutes` + stale display, and the app running card
  showing 600:01 for a stale timer (both Sol, with the 16:27 window); Pause could refetch status.
- 004 review (independent, APPROVE, 5f88483). For C5's next resume: (a) the foreign_key_check error
  names table, rowid, parent and fkid; (b) a full v1→v4 rotation test asserting kept backups are
  v1, v2, v3; (c) backlog work makes the codec's `block`/`start_day` Option and old item_history JSON
  still decodes; (g) polish: Backlog group-heading dots use a muted grey, not the project colour; (f) r3-findings.md (31 app/widget bugs, 8 high; review was cut off before DONE): app ones → an Astra app fixer after C7, plugin ones → C6; (e) fix all 8 findings in r2-findings.md (after resources, Astra); (d) `item.parse` understands "today"/"tomorrow" (sets the day, drops the word
  from the title) so the widget sheet and Enter fast path schedule tomorrow. Spec fix: 005 no longer re-creates `sessions_by_day` (001 has it).
- 17:09 reset 1 used by the orchestrator (user's OK) via the Codex TUI `/usage` → Redeem reset
  (tmux-driven). One reset left, expiring 13 Dec 02:46 (the used one would have expired 2 Dec), so
  resets do NOT expire on the 25 Nov renewal. Codex: fresh weekly + 5h at 17:09. Now: C5 resources
  (Sol, inherited a stopped Claude worker's partial diff), R2 review of epics/backlog (Astra).
- User 17:15: the Codex subscription ENDS on 25 Nov. Use the last reset as soon as the weekly hits
  100% (expected ~09:00 on the 24th: windows from 17:09, ~22:09, ~03:09, ~08:09), then windows ~09:10,
  ~14:10, ~19:10 and ~00:10 on the 25th until the subscription stops. Every window must start on time
  and be fully used (2-3 parallel workers on disjoint dirs).
- User 17:35: keep the rule, use the last reset only at a 100% weekly cap (option A). Billing period
  25 Oct – 25 Nov 2026 (Plus, paid 26 Oct). Measured: one 5h window ≈ 16% weekly (Sol-heavy), used in
  ~15 min by 3 workers. Expected weekly cap late on the 24th (windows 22:10, 03:10, 08:10, 13:10,
  18:10, 23:10), then reset and keep going on the 25th until the subscription ends.
- U5: user visual check after `omarchy restart shell`; fix what they report.
- Optional: app/widget could show `stale` on an overnight timer (backend reports it since 47fb1d1).
- Codex 5h limit hit at 06:59 (resets 11:26); weekly was 45% at 06:20.
- 22:30 render of c514311 (scratch daemon from HEAD, 2 overdue reminders): no bar badge, no Due
  section. `ResourceLogic.orderedDueReminders` uses `Array.isArray(rows)`, which is false for the
  QML-converted `status.reminders` (debug print: report.reminders has 2 rows, dueReminders 0).
  Send to C6 at 2b: accept any array-like (`rows && typeof rows.length === "number"`, copy with
  `Array.prototype.slice.call`) and add a node test with an array-like object.
- 22:28 Codex 5h cap (window opened 22:10; weekly 16→31%, so one window ≈ 15% weekly, burned in
  ~16 min by 3 workers). Next window 03:12. Committed: c514311 + d9b5f8f (C6 reminders, resources in
  rows), 9ceb3db (C5 R2 fixes, 214 tests incl. sockets), d7a3423 (C8 0a block_minutes, Claude Opus
  inherited), eb8bfb2 (C6 R3w widget fixes + reminder array-like bug, Claude Sonnet inherited;
  render shows 🔔 2 and the Due section). Claude now holds: C8 task 0 = migration 005 (Opus; then
  the independent review gate), C7 R3a app fixes (Opus, inheriting Codex's +801/−339 WIP).
- 0a decisions: leftover minutes use standard largest remainder (start block wins ties only), kept.
  `stats.get` `blocks` (app ChartsView "By prayer block") still counts sessions whole in their
  start block, so it can disagree with stats.table group "block": queue C8 0b = switch it to
  block_minutes (values change, no field change; note it in protocol-v1.md).
- 23:4x migration 005 (goals, reviews, item_tags seeded from live project tags) by Claude Opus,
  independent review APPROVE (Opus). Live copy v3→v5 in scratch: integrity/FK ok, every table's row
  count unchanged, item_tags = Dream Intensive|arabic, Mumtoz alifbo|arabic, CMU - DBMS homework 2|dmbs.
  INSTALL GATE: no build containing 005 goes onto the live DB until C8 0c's write side (item.add
  project defaults, item.edit tag_ids, tag.remove cleaning item_tags) is in the same install.
  0c must also: an RPC test of stats.table group "tag" on the v4 fixture before/after migrating
  (incl. the moved-task case). Goals/reviews write code must validate numbers and YYYY-MM-DD dates
  (review week = a Monday) since SQLite CHECKs don't enforce types.
- 23:5x committed 3adab2c (C7 task 1, app resource drawer, Claude Sonnet; 76 app tests; screenshots
  checked). Tree clean. Phase 3 and all review fixes are committed, NOT installed (install gate above).
  Queue for the 03:12 Codex window:
  C8 --resume (Astra): 0b (stats.get `blocks` via block_minutes) + 0c (tags on tasks; RPC test of
  tag totals before/after 005), then goals.
  After 0c: C9 0 (app tags, Astra/Sol) and C10 0 (widget tags, Sol) in parallel.
  Then the install: full scripts/test, live-copy migration diff, screenshots, ./scripts/install.
  Polish leftovers: Backlog heading dots in project colour; FilterBar task search box height.
