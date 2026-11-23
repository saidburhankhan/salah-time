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

- U5: user visual check after `omarchy restart shell`; fix what they report.
- Optional: app/widget could show `stale` on an overnight timer (backend reports it since 47fb1d1).
- Codex 5h limit hit at 06:59 (resets 11:26); weekly was 45% at 06:20.
