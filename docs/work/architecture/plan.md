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
