# Protocol v1: JSON-RPC 2.0 over NDJSON

This is the transport-independent contract introduced in S3. The same methods
and DTOs are served over stdin/stdout by `salah rpc` and over a user-only Unix
socket by `salah serve` (S4). The human `salah … --json` contract stays frozen
in [`contract-v1.md`](contract-v1.md); it is not the RPC contract.

## Transports

### stdio

`salah rpc` is the default, dependency-free transport: it reads request lines
on stdin and writes response lines on stdout until EOF. It handles requests
in-process, so it is the fallback when no daemon is running and the harness
used by tests and scripts.

`salah rpc --socket` is a stdio↔socket bridge: it connects to the running
daemon and copies frames in both directions, so scripts can keep speaking
NDJSON on stdin/stdout without knowing about Unix sockets.

### Unix domain socket

`salah serve` listens on:

```text
$XDG_RUNTIME_DIR/salah/salah.sock
```

`XDG_RUNTIME_DIR` is required; there is deliberately no `/tmp` fallback and the
daemon never listens on TCP. The runtime directory is mode `0700` and the
socket is mode `0600`, so only the same user can connect. On startup a live
socket is detected by connecting to it; if another daemon answers, the new
process refuses to start. A stale socket file is removed and replaced. On
SIGTERM/SIGINT the standalone daemon removes its socket.

The daemon is the single writer: every mutation is serialised through one
writer thread, while each connection reads through its own SQLite connection.
The single `salah` CLI never talks to the daemon, so the daemon also polls
`PRAGMA data_version` about every 250 ms and emits `changed` when a direct CLI
write advances the store revision.

### systemd user activation

`scripts/install` writes `~/.config/systemd/user/salah.socket` and
`salah.service` (ownership-checked and digested like other installed files)
and, for the real home only, enables the socket. The units are equivalent to:

```ini
# salah.socket
[Socket]
ListenStream=%t/salah/salah.sock
SocketMode=0600
DirectoryMode=0700

# salah.service
[Service]
ExecStart=%h/.local/bin/salah serve
```

The service accepts systemd's `LISTEN_FDS`/`LISTEN_PID` descriptor 3 as well as
binding the socket itself. A socket-activated instance exits after five idle
minutes without connections (configurable with `--idle-minutes`) so systemd can
restart it on demand; a manually started daemon stays up.

## Framing and handshake

- UTF-8, one compact JSON object per `\n`-terminated line, at most 1 MiB per
  line. Batch requests are not supported.
- A request is `{"jsonrpc":"2.0","id":1,"method":"…","params":{…}}`. The `id`
  is echoed unchanged. `params` may be omitted or `null` for no-argument
  methods; non-empty params must be an object.
- The first successful method on a connection must be `hello`:

```json
→ {"jsonrpc":"2.0","id":1,"method":"hello",
   "params":{"protocol":[1],"client":"widget/0.4","actor":"widget"}}
← {"jsonrpc":"2.0","id":1,
   "result":{"protocol":1,"server":"salah 0.1.0",
             "capabilities":["timer.v1"],"revision":184}}
```

Any other method before `hello` fails with JSON-RPC `-32600`. The server
supports protocol major `1`; if the client does not offer it, `hello` fails with
`-32602`. `capabilities` is additive: clients ignore names they do not know.
`events.v1` means the connection supports `subscribe` and `changed` pushes.

## Methods

Mutation parameters all include `op_id` (a stable client UUID, or a platform id
such as `telegram:<update_id>`). `revision` in a mutation result is the
store-wide counter, never a precondition. `changed.items` are canonical
`DayItem` rows plus the item `version`; `changed.deleted` lists removed ids.

| Method | Params | Result | Errors | Idempotent | Version-checked |
|---|---|---|---|---|---|
| `hello` | `protocol[]`, `client`, `actor?` | `protocol`, `server`, `capabilities`, `revision` | protocol | — | — |
| `status.get` | — | `Status` (today's widget projection, planner attached) | config, source, storage | — | — |
| `day.get` | `date?` | `DayPlan` | validation, source, storage | — | — |
| `items.list` | — | live `Item` rows plus `version` | storage | — | — |
| `stats.get` | `from,to` or `days?` | `Stats` | validation, source, storage | — | — |
| `history.get` | `from?`, `to?` (default last 7 days) | `[DayReport]` | validation, source, storage | — | — |
| `item.parse` | `text` | `NewItemDraft` | validation | — | — |
| `item.add` | `op_id`, `title` or `quick_text`, `block?`, `repeat?`, `weekdays?`, `measure?`, `target?`, `date?` | mutation result | validation, source, storage | yes | no |
| `item.edit` | `op_id`, `id`, `expected_version?`, `title?`, `block?`, `target?` (`null` clears), `repeat?`, `weekdays?`, `measure?` | mutation result | validation, not_found, conflict, storage | yes | yes |
| `item.move` | `op_id`, `id`, `block`, `after_id?` / `before_id?` (or legacy `index?`), `expected_version?` | mutation result | validation, not_found, conflict, storage | yes | yes |
| `item.remove` | `op_id`, `id`, `expected_version?` | mutation result (`changed.deleted`) | not_found, conflict, storage | yes | yes |
| `entry.done` | `op_id`, `item_id`, `date?` | mutation result | not_found, validation, storage | yes | no |
| `entry.undo` | `op_id`, `item_id`, `date?` | mutation result plus `undone` count | not_found, validation, storage | yes | no |
| `entry.log` | `op_id`, `item_id`, `kind?` (`count`/`minutes`), `value`, `date?` | mutation result | not_found, validation, storage | yes | no |
| `timer.start` | `op_id`, `item_id` | mutation result | not_found, validation, storage | yes | no |
| `timer.stop` | `op_id`, `item_id?` (omit to stop whichever runs) | mutation result | not_found, validation, storage | yes | no |
| `prayer.mark` | `op_id`, `prayer`, `date?`, `expected_version?` | mutation result (`changed.marks`) | validation, conflict, storage | yes | optional |
| `prayer.unmark` | same as `prayer.mark` | mutation result (`changed.marks`) | validation, conflict, storage | yes | optional |
| `config.get` | — | `Config` | config | — | — |
| `config.set` | `op_id?`, any `Config` field | `Config` | validation, config | yes with `op_id` | no |
| `export` | — | v3 `salah-export` document | storage | — | — |
| `import` | `data` (export document), `op_id?` | `revision`, `ImportSummary` | validation, storage | yes with `op_id` | no |
| `subscribe` | `topics?` (`["changes"]`), `after_revision?` | `revision`, `after_revision?`; only the socket transport then pushes `changed` | validation | — | — |

Ids are **full ids** over RPC. Prefix resolution remains a human-CLI
convenience only. A missing or stale entity is `not_found` or `conflict`.

## Errors

Application errors use fixed `-32000-n` codes and carry stable
`error.data.kind` values taken directly from `salah_domain::Error::code()`:

| Kind | Code | Retryable |
|---|---|---|
| `validation` | -32001 | no |
| `not_found` | -32002 | no |
| `ambiguous` | -32003 | no |
| `conflict` | -32004 | no |
| `busy` | -32005 | yes |
| `storage` | -32006 | no |
| `source_unavailable` | -32007 | yes |
| `config` | -32008 | no |
| `internal` | -32009 | no |

The top-level `message` is human text; clients branch on `data.kind`, not on
the message. Validation errors may add `data.field`. Protocol faults use the
JSON-RPC reserved codes: `-32700` parse, `-32600` invalid request, `-32601`
method not found, `-32602` invalid params, `-32603` internal serialization.
Protocol faults have no application `kind` (invalid params does carry
`kind:"validation"` so callers have one branch point).

Example stale edit:

```json
{"jsonrpc":"2.0","id":9,"error":{"code":-32004,
 "message":"item \"…\" is at version 4, not 3",
 "data":{"kind":"conflict","retryable":false}}}
```

## Idempotency and versions

- A repeated `op_id` returns the stored result and revision and performs no
  write, even if the retry body differs. On first application the planner
  records the complete serialized protocol result in `applied_ops` inside the
  same `BEGIN IMMEDIATE` transaction as the mutation. A replay returns those
  exact bytes, including the `changed` rows as they were then, even if later
  edits have changed the current state.
- `item.move` resolves `after_id`/`before_id` against the siblings read inside
  the same write transaction that reorders the block, so the anchor cannot go
  stale between the read and the move.
- `config.set` cannot share SQLite's transaction with the JSON config file. It
  writes the config file first through the repository's atomic rename and then
  records `op_id` in the same SQLite write transaction as `applied_ops`, so a
  replay returns the recorded `Config` without touching the file. If the
  process dies between the rename and the commit, the new file remains without
  an op row and a retry applies its body once more; the file is never left
  half-written.
- Reuse the same `op_id` when retrying after a timeout; use a new one for a new
  intent.
- `expected_version` applies per entity on `item.edit`, `item.move`,
  `item.remove` (and optionally prayer marks). Appends (`entry.*`, `timer.*`,
  `item.add`) need no precondition, only `op_id`. A stale version is
  `data.kind = "conflict"` and writes nothing.
- Clients obtain item versions from `items.list` or from the `version` field on
  mutation `changed.items`. `revision` orders pushes and de-duplicates them; it
  is never sent as a mutation precondition.

## Notifications

A client subscribes after `hello`:

```json
→ {"jsonrpc":"2.0","id":4,"method":"subscribe",
   "params":{"topics":["changes"],"after_revision":184}}
← {"jsonrpc":"2.0","id":4,"result":{"revision":184}}
```

The stdio transport accepts `subscribe` and returns the revision but has no
push channel. The socket daemon registers the connection and pushes every
revision advance, including those caused by a direct `salah` CLI write to the
same database.

Notifications have no `id`:

```json
{"jsonrpc":"2.0","method":"changed",
 "params":{"revision":185,"days":["2031-05-14"],
           "scopes":["items","entries"],"op_id":"…","actor":"widget"}}
```

`changed` is an invalidation, not a patch: refetch `status.get` or `day.get`.
`days` and `op_id` may be omitted; `scopes` is one of `items`, `entries`,
`marks`, `config`. The daemon emits these from its `data_version` watcher; a
notification may cover more than one commit observed in the same poll, so
clients treat its `revision` as the newest state to fetch and de-duplicate by
revision.

## Evolution rules

- Protocol `1` is additive: new optional fields, new methods, and new
  capability strings may appear. Clients ignore unknown fields and methods
  they do not call.
- Existing field names, types, meanings, method names, and error `kind`
  strings do not change within major `1`.
- A breaking change needs a new method name or a new major protocol version;
  the server may serve several majors.
- Enum values (`prayer`, `block`, `repeat`, `measure`, `state`, `kind`) are
  part of the contract. Additions require a contract review.
- The CLI v1 fixtures remain the compatibility suite for the human/script
  interface; they do not constrain the RPC wire format.
