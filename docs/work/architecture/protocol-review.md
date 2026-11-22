# Protocol review

Independent review of the communication and data protocols on `architecture`
(2026-11-22). It covers six protocol layers and goes against the earlier proposals
where they picked the easy option over the right one. Line numbers refer to the
working tree at review time, and other engineers are editing those files.

## Summary

| Layer | Current | Verdict | Recommended |
|---|---|---|---|
| 1 Client ↔ core transport | One `salah` process per action, JSON on stdout, 30 s polling | **Change, earlier than planned** | JSON-RPC 2.0 as NDJSON over a user-only Unix socket, served by `salah serve` with systemd user socket activation. `salah rpc` speaks the same frames over stdio. |
| 2 Message contract | Unwrapped v1 JSON. Errors are prose on stderr. Planned per-response envelope with a global `base_revision` | **Change** | JSON-RPC envelope with one `hello` handshake, string error kinds in `error.data`, client `op_id` idempotency, per-entity `version` preconditions, mutations that return the affected projection rows |
| 3 Optimistic UI | Planned journal with inverse patches and rebase | **Drop** | Server-authoritative: a pending flag per row, the canonical response, then a pushed `changed` notification |
| 4 Integrations | Planned transactional outbox and dispatcher | **Change** | One append-only `changes` log. Each consumer keeps its own cursor (at-least-once). Bots run as separate processes on the same socket. |
| 5 Sync | Snapshot export/import, last-writer-wins on wall-clock `updated_at` | **Change before any cloud work** | Hybrid logical clock (HLC) plus a device id per change. Items use per-field LWW. Entries are a grow-only set. Marks are LWW registers. Fractional order keys. No CRDT library. |
| 6 Storage concurrency | WAL, 3 s busy timeout, several writers, many mutations outside a transaction | **Fix now, then a daemon writer** | Every use case runs in one `BEGIN IMMEDIATE`. Reads stop writing. The daemon owns writes and detects external commits with `data_version`. |

## Evidence base

- **Quickshell on this machine (0.3.2).** `/usr/lib/qt6/qml/Quickshell/Io/quickshell-io.qmltypes` exports `Socket` (a `QLocalSocket` with `path`, `connected`, `write(QString)`, `flush()` and `parser`), `SocketServer`, `SplitParser` (with `splitMarker`), `DataStream`, `Process` (with `stdinEnabled` and `write`), `StdioCollector`, `FileView`, `JsonAdapter` and `IpcHandler`. Omarchy's own shell already runs a private socket protocol this way: `/usr/share/omarchy/shell/shell.qml:1656-1686` uses `SocketServer`, `Socket`, `SplitParser` and `write`/`flush` in `$XDG_RUNTIME_DIR`.
- **Missing from the widget runtime.** There is no generic D-Bus QML type; D-Bus appears only inside the DBusMenu, StatusNotifier and Mpris services. There is no WebSocket QML module (`qt6-websockets` is not installed). `QtNetwork` QML exports only `NetworkInformation`, `Ssl` and `SslSocket`. There is no Qt gRPC. `XMLHttpRequest` has no streaming, so SSE is not possible.
- **Qt C++.** `QLocalSocket` ships in qt6-base (QtNetwork), and `libQt6DBus` is present. The app needs `QT += network` (`app/salah-app.pro:1`).
- **Rust.** There is no async runtime (`Cargo.toml`). A std `UnixListener` with threads is enough.
- **Latency.** The installed `salah status --json` averaged about **1 ms** per call (20 runs). Spawn cost is not the problem. Push, idempotency and write ordering are.
- **systemd.** The user manager is running, so socket activation is available.

---

## 1. Client ↔ core transport

**Current.** `CoreClient` runs one `QProcess` per request (`app/src/client/coreclient.cpp:63-72`). The widget runs `Process` objects (`plugin/local.salah/BarWidget.qml:405-441`) and polls every 30 s (`:344-350`), once in every bar instance on every monitor. A timeout kills the child (`coreclient.cpp:81-85`) even if its write already committed. A retry then duplicates non-idempotent `done` and `log` calls (`src/planner/entries.rs:33-47`, where each call inserts a new UUID).

**Verdict: change.** Switch to a long-running per-user core speaking **JSON-RPC 2.0, one JSON object per `\n`-terminated line, over a Unix domain socket**.

| Option | Widget (Quickshell) | Qt C++ | Push | Security | Bots and other processes | Effort | Decision |
|---|---|---|---|---|---|---|---|
| CLI per action (today) | `Process` | `QProcess` | none (polling) | good (no listener) | spawn per call | 0 | keep only as a fallback and for humans |
| **(a) JSON-RPC over UDS** | **`Socket` + `SplitParser`, native** | `QLocalSocket` | notifications on the same connection | 0600 socket in `$XDG_RUNTIME_DIR`, `SO_PEERCRED` check, no network | any language, even `socat` | small (serde already present) | **adopt** |
| (b) D-Bus session service | no generic QML type, so it needs a `gdbus`/`busctl` `Process` bridge | QtDBus | signals | good | needs a session bus (fails in containers or headless bots) | medium (zbus plus a runtime). Nested DTOs end up as JSON strings inside D-Bus anyway. | reject |
| (c) HTTP/REST + WebSocket/SSE on localhost | `XMLHttpRequest` only: no WebSocket, no streaming | needs QtWebSockets (not installed) | not in the widget | TCP loopback is reachable by every local user and by browser pages (CSRF, DNS rebinding), so it needs tokens | good | medium | reject locally. HTTP is right only for a future sync server. |
| (d) gRPC | none | no Qt gRPC installed | streams | depends on setup | good | high (protobuf codegen in 3 languages, tonic+tokio) | reject |

Why (a) wins:

- It is the only option with native push in the widget.
- It serializes writes in one process.
- File permissions on the socket replace auth tokens.
- It reuses the existing serde DTOs.
- It follows the convention the host shell already uses.

**Lifecycle.** Install `salah.socket` and `salah.service` as systemd **user** units, using `ListenStream=%t/salah/salah.sock` and `SocketMode=0600`. Clients only need to connect; systemd starts the daemon on the first connect. If the socket is missing, clients fall back to `salah rpc`, which reads JSON-RPC lines on stdin and handles them in-process. `salah rpc` is also the test harness and the bridge for scripts.

**Do not adopt.** No TCP listener. No WebSocket or HTTP server locally. No D-Bus API, although calling `org.freedesktop.Notifications` as a client is fine. No gRPC/protobuf. No tokio unless a measured need appears; one writer thread plus one thread per connection is enough.

## 2. Message contract

**Current problems.**

- The typed error class exists (`src/error.rs:28-40`) but never reaches the wire: `main.rs:419-425` prints the prose message and exits 1. Clients get an error message and nothing to branch on.
- Mutations return incompatible shapes. `done` and `log` return a raw `Entry`, `undo` returns `{"undone":n}`, `stop` returns `[Entry]`, and `remove` returns the tombstoned `Item`. None of them returns the item's resulting `DayItem` state, so both UIs re-derive state. That is the root cause of the duplicated domain transitions in `review-frontends.md`.
- `find_item` accepts **id prefixes** (`src/planner/items.rs:28`). That is fine for humans, but dangerous for mutations sent by bots or other programs.
- `item move --index N` sends a stale numeric index.
- The planned envelope (`proposal-frontends.md`) puts `schema_version` and `capabilities` in every response and a **global** `base_revision` on every mutation. A global precondition conflicts on any unrelated write, such as the widget ticking an item while the app edits another one. That turns normal multi-client use into conflict handling.

**Verdict: change.** Use the JSON-RPC envelope and negotiate once per connection.

- **Request id** is the JSON-RPC `id`, unique per connection. It is used only for correlation.
- **Idempotency.** Every mutation carries `op_id`, a client UUID. Bots use the platform's update id instead, such as `telegram:<update_id>`. The core stores `(op_id → result, revision)` in `applied_ops` in the same transaction and replays the stored result on a repeat. Entries older than 7 days are pruned.
- **Optimistic concurrency** applies **per entity**. `item.edit`, `item.move` and `item.remove` take an optional `expected_version`. Appends (`entry.done`, `entry.log`, `timer.start`) need no precondition, only `op_id`. A stale version returns `conflict` along with the current entity.
- **Global `revision`** is a store-wide counter that advances on each commit. It is returned with every result and carried by every notification. Clients use it to order and de-duplicate pushes, not as a precondition.
- **Canonical mutation response:** `{revision, op_id, changed:{items:[DayItem…], marks:[PrayerMark…], deleted:[id…]}, …}`. The rows are exactly the projection the UI renders.
- **Errors:** `{"code":-32000-n,"message":"<user text>","data":{"kind":"not_found","retryable":false,"field":…,"current":…}}`. `data.kind` is the stable contract and uses the existing `Error::code()` strings. Numeric codes are fixed per kind, and JSON-RPC's reserved codes cover protocol faults.
- **Ids are full ids over RPC.** Prefix resolution stays in the human CLI.
- **Versioning:** `hello` returns `protocol` (an integer major) plus `capabilities` (strings such as `"events.v1"` or `"timer.v1"`). Within a major version, changes are additive only, and clients ignore unknown fields. A breaking change ships as a new method name or a new major version; the server can serve several. Keep the CLI v1 golden fixtures as the compatibility suite for human and script use only.

### Spec sketch (protocol 1)

Framing: UTF-8, one compact JSON object per line, maximum 1 MiB per line. Set `splitMarker: "\n"` explicitly in QML.

```text
→ {"jsonrpc":"2.0","id":1,"method":"hello","params":{"protocol":[1],"client":"widget/0.4","actor":"widget"}}
← {"jsonrpc":"2.0","id":1,"result":{"protocol":1,"server":"salah 0.2.0","capabilities":["events.v1"],"revision":184}}
```

| Method | Params | Result |
|---|---|---|
| `hello` | `protocol[]`, `client`, `actor` | `protocol`, `capabilities`, `revision` |
| `status.get` | — | today's widget projection (replaces `status --json`) |
| `day.get` | `date?` | `DayPlan` |
| `items.list` | — | `[Item]` |
| `stats.get` | `from,to` \| `days` | `Stats` |
| `item.parse` | `text` | `NewItemDraft` |
| `item.add` | `op_id`, fields \| `quick_text`, `date?` | mutation result |
| `item.edit` | `op_id`, `id`, `expected_version?`, changed fields | mutation result |
| `item.move` | `op_id`, `id`, `block`, `after_id?`/`before_id?`, `expected_version?` | mutation result |
| `item.remove` | `op_id`, `id`, `expected_version?` | mutation result |
| `entry.done` / `entry.undo` | `op_id`, `item_id`, `date?` | mutation result |
| `entry.log` | `op_id`, `item_id`, `kind:"count"\|"minutes"`, `value`, `date?` | mutation result |
| `timer.start` / `timer.stop` | `op_id`, `item_id?` | mutation result |
| `prayer.mark` / `prayer.unmark` | `op_id`, `prayer`, `date?` | mutation result (already idempotent through deterministic ids) |
| `config.get` / `config.set` | `op_id` on set | `Config` |
| `subscribe` | `topics:["changes","clock"]`, `after_revision?` | `{revision}`, followed by notifications |
| `events.read` | `consumer`, `after_seq`, `limit` | `[Event]` (integrations) |
| `events.ack` | `consumer`, `seq` | `{}` |

Notifications have no `id`:

- `changed {revision, days:[…], scopes:["items","entries","marks","config"], op_id?, actor}` is an invalidation, not a patch. Clients refetch `status.get` or `day.get`, which takes about 1 ms. A notification that carries the client's own `op_id` is redundant with the response, and its `revision` makes it idempotent.
- `clock.boundary {revision, boundary, at}` is sent by the daemon's scheduler at each prayer/block boundary and timer auto-close. It replaces the widget's refresh at zero and its 30 s poll.

A notification that arrives after a reconnect carries an `after_revision` gap, and the client simply refetches. Notifications contain no fine-grained entity diffs. Diffs would force clients to rebuild the projection, which is the duplication being removed.

## 3. Optimistic UI protocol

**Planned:** a journal of `{op_id, base_revision, patch, inverse}` with rollback and rebase, implemented twice, in C++ and in QML (`proposal-frontends.md` steps 1–7).

**Verdict: drop.** A round trip is about a millisecond, and the core will return canonical rows. A client-side patch/inverse/rebase engine is exactly the "UI as a second domain model" that the front-end review condemns. It would be built twice and is hard to test (the widget has no behavioural tests).

**Replacement:**

1. On an intent, mark the row `pending` (presentation only, such as a dimmed checkbox) and send the request with an `op_id`.
2. On the response, replace the affected rows with `changed`.
3. On an error, clear `pending` and show a row-level error with a retry. The retry reuses the same `op_id`, so it is safe after a timeout.
4. On `changed` from another actor, refetch the view.

Keep requests in order on one connection, so there is no need to rebase. Only a pending create needs a temporary client row keyed by `op_id`, and the response maps it to the real id.

## 4. Integration protocol

**Planned:** a domain event plus per-consumer outbox rows written transactionally, and a dispatcher with retry and backoff inside the core (`proposal.md` "Ports and transaction model", steps 11–12).

**Verdict: change.**

- **One log, many cursors.** Each commit appends rows to a `changes` table: `seq INTEGER PRIMARY KEY`, `event_id`, `type`, `at` (HLC), `device`, `actor`, `op_id`, `subject`, `data`. It is written in the same transaction as the mutation. That single log feeds push (`changed`), integrations (`events.read`/`ack` with a stored cursor per consumer) and sync. Outbox rows per consumer and a core-side dispatcher are unnecessary: a consumer that crashes re-reads from its cursor, which gives at-least-once delivery for free.
- **Event schema** (semantic, versioned per type): `{"v":1,"seq":912,"id":"uuid","type":"item.completed","time":"…Z","actor":{"kind":"telegram","id":"chat:42"},"op_id":"telegram:8812","subject":"item/<id>","data":{"item":{…DayItem},"day":"2026-11-22"}}`. Types include `item.created`, `item.updated`, `item.moved`, `item.removed`, `item.completed`, `item.reopened`, `entry.logged`, `timer.started`, `timer.stopped`, `prayer.marked` and `prayer.unmarked`. Time-driven reminders are **not** stored events; they come from `clock.boundary` notifications. A reminder missed while a bot was offline is stale anyway.
- **Delivery:** at-least-once. Consumers de-duplicate side effects by `event.id`.
- **Inbound commands:**
  - A bot is a **separate process** that translates `/done quran` into `entry.done` with `op_id = "telegram:<update_id>"` or `"slack:<event_id>"`. Platform redeliveries (Telegram until the offset is confirmed, Slack retries) then become no-ops through `applied_ops`.
  - Item references from chat are resolved through a `item.find` query that requires a unique match. Never guess.
- **Transport for bots:** use Telegram `getUpdates` long polling and Slack Socket Mode. These are outbound-only connections, so no public webhook is exposed on a desktop.
- **Auth and secrets:**
  - Bot tokens live with the bot, in a 0600 config file, systemd `LoadCredential=` or the Secret Service. They never go into `salah.db`, exports, events or logs.
  - An allow-list of chat and user ids lives in the bot.
  - The core trusts same-uid socket peers and records `actor`.
  - Least privilege comes later: if a bot must be read-only, give it a second socket (`salah-ro.sock`) that only serves queries. Do not build a token or ACL system until a second user exists.
- **Notes apps:** one-way rendering (a daily-note file written from `day.get` on `changed`). Do not build a two-way sync with Markdown.

## 5. Sync protocol (future cloud)

**Current:** a full snapshot export and import (`src/export.rs:121-178`) that overwrites a row when `incoming.updated_at > current.updated_at` (`:147,160,173`). Timestamps come from the wall clock of each device. This model has five defects:

1. **Equal timestamps with different content never converge.** The strict `>` keeps each side's version.
2. **Clock skew lets a stale device win.**
3. **Whole-row LWW** loses concurrent edits to different fields of one item.
4. **`move_item` renumbers every sibling** (`src/planner/items.rs:178-200`). Two devices reordering the same block produce an interleaved order that neither user made.
5. **No device identity and no change feed.** Every sync sends everything.

Merge model per entity:

| Entity | Nature | Merge rule |
|---|---|---|
| Item | Mutable record with a few independent fields | **Per-field LWW**, keyed by `(hlc, device)`, over these field groups: `title`, `block`, `schedule`(repeat, weekdays, day), `measure`(measure, target), `order_key`, `deleted`. A delete wins over a concurrent edit (keep the tombstone; no resurrection). |
| Entry (done, count, minutes, timer) | **Append-only fact** | Grow-only set union by id. `deleted_at` is monotone: once set it stays set, and the earliest value wins. A timer's `ended_at` is set once, earliest value wins. **"At most one running timer"** is enforced after the merge by a deterministic repair: the older timer is closed at the newer timer's start. |
| Prayer mark | Boolean per (day, prayer) with a deterministic id `day:prayer` (`src/item.rs:197`) | LWW register keyed by `(hlc, device)` |
| Order within a block | — | Use a **fractional index key** (a string sorted lexicographically, ties broken by id) instead of dense `position` integers, so a move writes one field of one row |

**Recommendation: the simplest correct model.**

- Use an HLC stored as a 64-bit integer: physical milliseconds shifted left 16 bits, plus a logical counter. Each device gets a stable `device_id`.
- Use the per-entity rules in the table. Every local change already lands in the `changes` log from layer 4, so sync is "push my changes since cursor X, pull theirs since cursor Y, apply the merge function".
- The server is a dumb, ordered relay storing `(device, seq, ciphertext)`. Payloads should be **end-to-end encrypted**, because prayer and habit records are sensitive. The server needs no knowledge of the domain.
- Export/import stays as a backup format and goes through the **same** merge function.
- Introduce the HLC, device id and field versions **when** the `version`/`changes` columns for layer 2 are added, so the schema migrates once.

**Do not adopt.** No CRDT library (Automerge or Yjs). There is no collaborative text, and it would make SQLite query models opaque. No event sourcing as the system of record. No vector clocks (the device count is small and LWW plus HLC is enough). No wall-clock LWW.

## 6. Storage concurrency

**Current.**

- WAL is on (`src/store.rs:192`) with a 3 s busy timeout (`:202`), and `transaction()` uses `BEGIN IMMEDIATE` (`:240`). These are good.
- Most use cases read outside any transaction and then write:
  - `add` computes `next_position` from a full scan, then calls `put_item` with no transaction (`items.rs:16-24,104-110`). Two concurrent adds get the same position.
  - `move_item` reads siblings at `:178` but opens its transaction at `:191` (time-of-check to time-of-use).
  - `edit` is a read-modify-write with no transaction (`:124-163`), so concurrent edits lose updates.
  - `start_timer` closes timers and inserts in separate transactions (`timers.rs:49-58`).
- **Reads write.** `day`, `item list`, `add` and others call `close_expired_timers` first (`src/main.rs:577,761` and elsewhere). As a result, every poll from every widget instance may take the write lock.

**Fix now (without a daemon).** Each use case runs as a single `store.transaction` with `BEGIN IMMEDIATE` around its reads and writes. Set `synchronous=NORMAL` (safe in WAL). Map `SQLITE_BUSY` to the existing retryable `Error::Busy`. Make the `day`/`status` queries compute expired timers as *ended at block end* in the projection without persisting, and persist the closure on the next mutation.

**With the daemon.**

- One writer thread owns the write connection and applies commands in order.
- A small pool of read connections serves queries.
- A scheduler persists timer auto-closure at block ends and emits `clock.boundary`.
- The CLI may keep writing directly; SQLite transactions plus revision checks keep that correct. The daemon detects foreign commits with `PRAGMA data_version`, checked on a 1 s tick or on inotify of `salah.db-wal`, and broadcasts `changed`. This is more robust than the earlier rule that the fallback must never write: a dead daemon never blocks a human at a terminal.
- Keep the busy timeout for that case.

---

## Impact on `plan.md`

Ordered replacement for P3 onwards. P1 and P2 stay as done.

1. **P3, reworked.** Make every mutation atomic as described in layer 6, stop reads from writing, and keep the repository/unit-of-work. Do this first; it is a correctness fix today.
2. **New: protocol schema migration.** Add `revision`, a per-entity `version` (HLC plus device), `order_key` (fractional, migrated from `position`), field versions for items, the `changes` log and `applied_ops`. This is the core 11 schema pulled forward. It **drops the per-consumer outbox and dispatcher**.
3. **Core 8, reworked and pulled forward.** The contract becomes JSON-RPC method DTOs, error kinds, canonical mutation results and `item.move {after_id|before_id}`, plus `salah rpc` (stdio). **Drop** `salah api --version 1 <operation>` and the per-response `schema_version`/`base_revision` envelope.
4. **Core 13 and FE 11, moved here from P8.** Add `salah serve` with systemd user socket activation, `subscribe`/`changed`/`clock.boundary`, the `data_version` watcher and a boundary scheduler. Include a load test with one writer, N subscribers and an external CLI writer.
5. **FE 3 follow-up.** Add a `SocketTransport` (`QLocalSocket`, `QT += network`) behind `CoreClient`, with `CliTransport` reduced to a `salah rpc` fallback. Remove the reload of three commands per refresh.
6. **FE 7, merged with the widget transport.** Run the controller on Quickshell `Socket` plus `SplitParser`, remove the 30 s poll and the refresh at zero, and keep the 1 s countdown display only.
7. **FE 4 and FE 6** (typed models and domain projections in Rust) are now built on push and canonical rows. Many UI-side transitions are deleted rather than ported.
8. **FE 8 optimistic journal: drop.** Replace it with the pending flag and canonical response from layer 3, which needs about a day per client.
9. **Core 12 proof integration.** Build a Telegram bot as a separate process: `getUpdates`, then `entry.done` with `op_id = telegram:<update_id>`, plus `events.read`/`ack` for notifications. Prefer it over a notes adapter because it exercises inbound idempotency.
10. **Core 6–9 (workspace and adapter crates) and core 10 (IANA zones)** are off the critical path for the protocol. Run them in parallel after step 3. IANA zones must land before the boundary scheduler is used outside fixed-offset regions.
11. **Sync (new, last).** Build the HLC merge function, apply it to import/export first, then the encrypted relay.
