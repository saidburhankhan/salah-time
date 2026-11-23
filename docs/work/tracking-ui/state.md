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
  1. 16:27: C5 task 0 = migration 004 (Astra, crates; Claude review gate), C4 tasks 4-5 = general
     table + Notes page (Astra, app; needs C2 3b), widget stale-timer display (Sol, plugin).
  2. After reset 1: C5 rest (backlog/epic rules Astra; resources, CLI, fixtures Sol), C6 widget
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
  When Codex hits a cap: Claude subagents inherit (Opus hard, Sonnet normal) and the orchestrator
  tells the user so they can apply a reset.
- U5: user visual check after `omarchy restart shell`; fix what they report.
- Optional: app/widget could show `stale` on an overnight timer (backend reports it since 47fb1d1).
- Codex 5h limit hit at 06:59 (resets 11:26); weekly was 45% at 06:20.
