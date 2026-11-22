# State: tracking-model

## Repository

- Branch: `architecture`. Backend: S1 7d1299e, S2 bf4de55, S3 241326d c1753ec, S4 4b1c5b7
  dbbf021, review fixes 496da37. Frontend S5: bb419b4.

## Outcome

Complete and installed (2026-11-23 01:19). Projects, tags, metrics (built-in focus and quality,
custom metrics from settings), sessions with metric values, day plans with capacity and
fulfilment, and stats by tag/project/block/day. The app and widget use the new protocol.

## Verification

- `./scripts/test` green: 125 Rust tests including daemon sockets, 0 ignored; app tests 45/45
  (48 test functions incl. helpers), run outside the Codex sandbox; plugin node checks 29/29.
- `cargo clippy --offline --all-targets -- -D warnings` clean; `git diff --check` clean.
- `./scripts/install` ok; `omarchy plugin validate` ok on the installed plugin; daemon
  restarted on the new binary; `salah status --json` ok; no warnings in the user journal.
- End to end on a scratch database: a project asking focus, quality and a custom count metric;
  logging without the required values is rejected naming the missing ones; a full log reads back
  intact; stats show the custom series and totals.

## Not verified

- Visual check of the widget and app (needs `omarchy restart shell` and a look by the user).

## Known gaps

- The missing-metrics CLI message names custom metrics by id, not name.
- The first item write after a long gap writes up to 366 day snapshots.
- A running timer is not auto-closed; its minutes are only clamped to the day's Sleep boundary.
- `docs/work/architecture/contract-v1.md` predates schema v1; `protocol-v1.md` is current.
