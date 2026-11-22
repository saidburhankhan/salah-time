# Plan: architecture

## Outcome

Rebuild Salah as a layered platform without changing user-visible behaviour:
enforced dependency direction (Cargo workspace), typed errors, injected
clock/ids/prayer source, a versioned contract, one source of truth for
domain rules (in Rust), typed front-end models and split QML, then events,
an outbox, and a daemon so integrations (notes, Telegram, Slack) and sync
plug in. Inputs: `review.md`, `proposal.md`, `review-frontends.md`,
`proposal-frontends.md` in this directory.

## Rules

- Branch `architecture`. One small commit per step; `./scripts/test`,
  strict Clippy, and `app/bin/test` stay green after every step.
- No behaviour change unless a step says so; contract fixtures (P1) guard it.
- Workers own disjoint files per step; Cargo workspace edits are serialised
  by the lead.
- The lead reviews by tests, diff stats, and short reports, not full reads.

## Phases (revised 2026-11-22 after `protocol-review.md`)

The protocol review replaced the easy choices with proper ones: JSON-RPC 2.0
(NDJSON) over a 0600 Unix socket served by `salah serve` (systemd user socket
activation; `salah rpc` stdio fallback), per-entity versions + client op ids,
mutations returning canonical rows, pushed `changed` notifications instead of
polling, one append-only `changes` log with per-consumer cursors, HLC-based
sync last. The optimistic journal (FE 8) is dropped.

Done: P1 fixtures + typed errors; P2 planner split, Clock/IdGenerator,
desktop CoreClient, app DTO decoders.

| Phase | Work | Worker |
|---|---|---|
| S1 Storage correctness | Every mutation in one `BEGIN IMMEDIATE` transaction; reads never write (no `close_expired_timers` on read paths; derive expiry); busy handling | Codex |
| S2 Schema | `revision`, per-entity `version`, `changes` log, `applied_ops` (idempotency); migration; export v3 | Codex |
| S3 Contract | JSON-RPC method/notification DTOs, error `kind` on the wire, `hello` handshake, mutations return changed rows + revision; CLI JSON keeps working | Codex |
| S4 Daemon | `salah serve` (single writer, `PRAGMA data_version` watch for direct CLI writes), `salah rpc`, systemd user socket units, installer | Codex |
| S5 Clients | App CoreClient → socket with subscriptions; widget → Quickshell `Socket`; remove polling and client re-derivation | Codex (app) ∥ DeepSeek (widget) |
| S6 Front-end structure | Typed models, split QML, domain projections to Rust, i18n/a11y | DeepSeek ∥ Codex |
| S7 Workspace | Crates (domain, prayer-times, contract, adapters) with enforced dependency direction; IANA zones | DeepSeek / Codex |
| S8 Integrations | Telegram bot over the `changes` cursor (proof), then notes/Slack | DeepSeek |
| S9 Sync | HLC + device id, per-entity merge, fractional order keys; relay later | Codex |

## Acceptance

- [ ] Every finding in both reviews is fixed or explicitly deferred with reason.
- [ ] Workspace crates with dependency direction enforced by Cargo.
- [ ] No business rules duplicated in QML/C++ (recurrence, dates, ordering, timers).
- [ ] Contract fixtures pass in Rust, app, and widget decoders.
- [ ] All tests and strict Clippy green; installed and checked live by the user.

## Worker assignment (token budget)

DeepSeek (`dsh`, headless) takes mechanical, well-specified steps; Codex takes
design-heavy or high-risk steps. Every worker gets a narrow brief with owned
files and required checks; the lead verifies with tests and diff stats.

| Step | Worker | Why |
|---|---|---|
| P1 contract fixtures + consumer tests | DeepSeek | Capture current outputs; mechanical |
| P1 typed errors | Codex | Error taxonomy design |
| P2 split planner.rs behind facade | DeepSeek | Mechanical move, tests guard it |
| P2 Clock/IdGenerator injection | DeepSeek | Mechanical threading |
| P2 desktop CoreClient | Codex | C++ async design |
| P3 repository/unit-of-work | Codex | High-risk persistence |
| P4 workspace + crate extraction | DeepSeek (lead serialises Cargo edits) | Mostly file moves |
| P4 contract crate + v1 envelope | Codex | API design |
| P5 adapters | DeepSeek | Moves behind defined traits |
| P5 IANA zones | Codex | Correctness-critical time logic |
| P6 typed models / domain projections | Codex | Cross-layer design |
| P6 split QML (app, widget) | DeepSeek | Mechanical component extraction |
| P7 i18n / packaging | DeepSeek | Mechanical |
| P7 optimistic journal | Codex | Protocol semantics |
| P8 events/outbox, daemon | Codex | Architecture-critical |
| P8 proof integration adapter | DeepSeek | Built on defined ports |

If DeepSeek's result fails review twice, the step moves to Codex.
