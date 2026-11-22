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

## Phases (core step numbers from proposal.md, front-end from proposal-frontends.md)

| Phase | Steps | Parallel lanes |
|---|---|---|
| P1 Guard rails | core 1 + FE 1 (contract fixtures, consumer tests); core 2 (typed errors) | fixtures ∥ errors |
| P2 Seams | core 3 (split planner.rs), core 4 (Clock/IdGenerator); FE 3 (desktop CoreClient) | core ∥ app client |
| P3 Persistence | core 5 (repository/unit-of-work, atomic invariants) | serial |
| P4 Workspace | core 6 (salah-domain crate), 7 (prayer-times), 8 (contract crate + v1 envelope); FE 2 (decoders) | 7 ∥ 8 after 6 |
| P5 Adapters & zones | core 9 (SQLite/official/location adapters), 10 (IANA zones) | adapters ∥ |
| P6 Front ends | FE 4 (typed models), 5 (split QML), 6 (domain projections to Rust), 7 (widget controller/components) | app ∥ widget |
| P7 Reliability | FE 8 (optimistic journal), FE 9 (i18n/a11y), FE 10 (packaging) | ∥ |
| P8 Platform | core 11 (events/outbox), 12 (proof integration adapter), 13 + FE 11 (salahd daemon), core 14 | serial |

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
