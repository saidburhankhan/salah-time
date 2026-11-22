# State: sleep-boundary

## Repository

- Branch: `main`
- Dirty files before this work: `docs/AGENTS.md`, `docs/START.md`, `docs/work/`

## Now

Implementation is complete and installed. Sleep is a validated `HH:MM`
boundary (default `23:00`). A bedtime earlier than Fajr (e.g. `00:30`) belongs
to the previous evening (`day::evening_sleep_offset`). Status exposes `next`,
`current_block`, and a `boundaries` list that keeps Sunrise and lists Sleep
last. The installed plugin reloaded without QML warnings.

## Next

Archived 2026-11-22. Code verified and installed; the evening (after-Isha and
sleeping) bar views remain to be seen live by the user.

Previous next step: 
Visually inspect the bar and popup after Isha (counting down to Sleep) and
after the sleep time (`Sleeping · Fajr …`). Then archive this work item.

## Interfaces

- Existing `status --json` consumers retain `next.name`, `next.time`, and
  `next.remaining_seconds`.
- New boundary metadata is additive.

## Risks and blockers

- The awake block before a post-midnight bedtime starts at yesterday's Isha,
  approximated with today's Isha time (differs by about a minute).

## Verification (2026-11-22)

- `./scripts/test`: passed (31 unit, 7 CLI, 3 config tests, installer checks).
- `cargo clippy --offline --all-targets -- -D warnings`: passed.
- `git diff --check`: passed.
- `./scripts/install`, then `omarchy plugin validate ~/.config/omarchy/plugins/local.salah`: passed.
- `salah status --json` (12:14): next Asr, block Dhuhr→Asr, rows include Sunrise and Sleep 23:00.
- `journalctl --user`: plugin reloaded, no Salah/QML warnings.
- Not yet observed live: after-Isha and sleeping presentation.
