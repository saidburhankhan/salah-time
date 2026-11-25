# State: architecture

## Repository

- Branch: `architecture`, merged into `main` on 2026-11-25 (fast-forward).

## Now

Complete for S1–S7, delivered through the tracking-model and tracking-ui work: one write
transaction per mutation, schema with revision/versions/`changes`/`applied_ops`, the JSON-RPC
contract (`hello`, typed errors, mutations returning changed rows), the single-writer daemon
`salah serve` with systemd socket activation and `salah rpc`, app and widget clients on the
socket, typed app models and split QML, and the crate split (domain, prayer, store, core,
protocol, cli). Protocol reference (living): `docs/protocol-v1.md`.

## Next

None. S8 (integrations, e.g. a Telegram bot over the `changes` cursor) and S9 (sync) were not
started; they need a new work item if the user asks for them.
