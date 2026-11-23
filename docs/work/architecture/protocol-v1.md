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
             "capabilities":["tracking.v1","events.v1","clock.v1"],"revision":184}}
```

Any other method before `hello` fails with JSON-RPC `-32600`. The server
supports protocol major `1`; if the client does not offer it, `hello` fails with
`-32602`. `capabilities` is additive: clients ignore names they do not know.
`tracking.v1` means the projects, sessions, and metrics surface below.
`events.v1` means the connection supports `subscribe` and `changed` pushes.
`clock.v1` means the daemon can push `clock.boundary` (see Notifications).

## Methods

Mutation parameters all include `op_id` (a stable client UUID, or a platform id
such as `telegram:<update_id>`). `revision` in a mutation result is the
store-wide counter, never a precondition.

| Method | Params | Result | Errors | Idempotent | Version-checked |
|---|---|---|---|---|---|
| `hello` | `protocol[]`, `client`, `actor?` | `protocol`, `server`, `capabilities`, `revision` | protocol | — | — |
| `status.get` | — | `Status`: prayer projection, `planner` (today's rows), `running: SessionView\|null`, `prompt: [PromptMetric]` | config, source, storage | — | — |
| `day.get` | `date?` | `DayPlan`: prayers, blocks with `capacity_minutes`/`planned_minutes`/`actual_minutes`/`unplanned_minutes` and rows, `open`, `fulfilment` | validation, source, storage | — | — |
| `items.list` | — | `[ItemView]` | storage | — | — |
| `stats.get` | `from,to` or `days?` (default 30), `tag_id?`, `project_id?` | `range`, `filter`, `days` (with `unplanned_minutes`, nullable `fulfilment_ratio`), `series` (minutes first), `projects`, `tags`, `blocks`, `totals` | validation, not_found, storage | — | — |
| `item.parse` | `text` | draft `{title, block, repeat, weekdays, planned_minutes}` | validation | — | — |
| `item.preview` | `block`, `repeat`, `weekdays?`, `planned_minutes?`, `date` | `next_occurrence`, `occurrences_this_month`, `schedule_text` | validation | — | — |
| `item.add` | `op_id`, `title` or `quick_text`, `block?`, `repeat?`, `weekdays?`, `project_id?`, `planned_minutes?`, `date?` | mutation result (`changed.items`) | validation, not_found, source, storage | yes | no |
| `item.edit` | `op_id`, `id`, `expected_version?`, `title?`, `block?`, `repeat?`, `weekdays?`, `project_id?`, `planned_minutes?` (`null` clears either) | mutation result | validation, not_found, conflict, storage | yes | yes |
| `item.move` | `op_id`, `id`, `block`, `after_id?` / `before_id?` / `index?`, `expected_version?` | mutation result (every renumbered item) | validation, not_found, conflict, storage | yes | yes |
| `item.remove` | `op_id`, `id`, `expected_version?` | mutation result (`changed.deleted`) | not_found, conflict, storage | yes | yes |
| `projects.list` | `archived?` | `[Project + tags:[Tag] + metrics:[ProjectMetric + metric:Metric]]` | storage | — | — |
| `project.add` | `op_id`, `name`, `color?`, `note?`, `tag_ids?`, `metrics?:[{metric_id, required}]` (default focus and quality) | mutation result (`changed.projects`) | validation, not_found, storage | yes | no |
| `project.edit` | `op_id`, `id`, `expected_version?`, any add field (`note: null` clears), `archived?` | mutation result | validation, not_found, conflict, storage | yes | yes |
| `tags.list` | — | `[Tag]` | storage | — | — |
| `tag.add` | `op_id`, `name`, `color` | mutation result (`changed.tags`) | validation, storage | yes | no |
| `tag.edit` | `op_id`, `id`, `expected_version?`, `name?`, `color?` | mutation result | validation, not_found, conflict, storage | yes | yes |
| `tag.remove` | `op_id`, `id`, `expected_version?` | mutation result (`changed.deleted`, detached `changed.projects`) | not_found, conflict, storage | yes | yes |
| `metrics.list` | `archived?` | `[Metric]`, built-ins first | storage | — | — |
| `metric.add` | `op_id`, `name`, `unit?`, `kind` (`count`/`scale`/`check`), `scale_max?` (scale: 2–10), `rollup?` (default `avg` for a scale, else `sum`) | mutation result (`changed.metrics`) | validation, storage | yes | no |
| `metric.edit` | `op_id`, `id`, `expected_version?`, `name?`, `unit?`, `archived?` (a built-in cannot be archived; `kind` never changes) | mutation result | validation, not_found, conflict, storage | yes | yes |
| `session.start` | `op_id`, `item_id?` / `project_id?` | mutation result (`changed.sessions`) | validation, not_found, conflict (one already runs), storage | yes | no |
| `session.stop` | `op_id`, `id?` (default: the running one), `values?:{metric_id: number}`, `note?` | mutation result | validation, not_found, storage | yes | no |
| `session.log` | `op_id`, `item_id?` / `project_id?`, `date?`, `block?`, `minutes?` (none: a tick), `values?`, `note?` | mutation result | validation, not_found, storage | yes | no |
| `session.edit` | `op_id`, `id`, `expected_version?`, `minutes?`, `values?`, `note?` (`null` clears), `block?` | mutation result | validation, not_found, conflict, storage | yes | yes |
| `session.remove` | `op_id`, `id`, `expected_version?` | mutation result (`changed.deleted`) | not_found, conflict, storage | yes | yes |
| `prayer.mark` | `op_id`, `prayer`, `date?`, `expected_version?` | mutation result (`changed.marks`) | validation, conflict, storage | yes | optional |
| `prayer.unmark` | same as `prayer.mark` | mutation result (`changed.marks`) | validation, conflict, storage | yes | optional |
| `config.get` | — | `Config` | config | — | — |
| `config.set` | `op_id?`, any `Config` field | `Config` | validation, config | yes with `op_id` | no |
| `subscribe` | `topics?` (`["changes"]`, `["clock"]`, or both), `after_revision?` | `revision`, `after_revision?`; the socket transport then pushes `changed` and/or `clock.boundary` | validation | — | — |

The `entry.*`, `timer.*`, `history.get`, `export`, and `import` methods of the
first planner model are gone and answer `-32601`. The golden responses of every
method live in `crates/salah-cli/tests/fixtures/rpc/` and are the reference
shapes for clients.

A mutation result is `{revision, op_id, changed}`. `changed` always carries
every list — `items` (`ItemView`: the stored item plus `schedule_text` and
`next_occurrence`), `sessions` (`SessionView`), `projects`, `tags`, `metrics`,
`marks`, and `deleted` (removed ids) — so clients never guard for a missing key.

## Shapes

- `SessionView`: `id`, `item_id`, `project_id`, `day`, `block`, `started_at`,
  `ended_at`, `minutes` (`null` while running and for a tick), `running`, `stale`,
  `note`, `values` (`{metric_id: number}`), `version`. A session of an item
  copies the item's project.
- `status.get.prompt` lists the running session's active project metrics in the project's
  configured order. A running routine without a project gets focus and quality. Idle status
  returns an empty list. Each `PromptMetric` carries `metric_id`, `name`, `unit`, `kind`,
  `scale_max` and `required: false`. The `required` field on `ProjectMetric` is retained
  for compatibility and is not enforced. All session metrics and notes are optional.
- Day rows (`day.get` blocks and `status.get` planner): `id`, `title`, `block`,
  `repeat`, `weekdays`, `position`, `version`, `project` (`{id,name,color}` or
  `null`), `planned_minutes` (the day's snapshot), `actual_minutes`,
  `sessions`, `done` (the item has a live tick that day),
  `schedule_text`, `next_occurrence`, and `detail_text` (`"20 / 30 min ·
  Daily"`). Running sessions add no minutes until they stop. A row's `project`
  is the project as planned that day.
- `fulfilment`: `planned_items`, `done_items`, `planned_minutes`,
  `actual_minutes` (only sessions of that day's planned items) and
  `unplanned_minutes` (every other session: unplanned items, bare projects).
  Each day block carries `capacity_minutes`, `planned_minutes`,
  `actual_minutes` and `unplanned_minutes` the same way.
- `stats.get`: only finished sessions count. An `avg` rollup is weighted by
  session minutes, a tick weighing 1. Each `days[]` row is `{day, minutes,
  capacity_minutes, planned_minutes, unplanned_minutes, fulfilment_ratio}`;
  `fulfilment_ratio` is plan-line minutes over planned minutes, capped at 1,
  and `null` when nothing was planned. A day's `capacity_minutes` leaves out
  the night block (sleep to Fajr); per-block capacity still lists it.
- Snapshots: days from today on are computed live from the current config and
  items and are never stored. Once a day is past, its blocks and plan are
  written once (from the items that existed by its end, with their project
  then) and marked written, so an empty plan stays empty. A past day's prayer
  times and sleep time come from its stored blocks. Item writes first freeze
  every unwritten past day, so later edits never reach them. Snapshots are not
  entities: they neither bump the revision nor appear in `changed`.
- `item.add` rejects a `date` before today.
- A bare `session.stop` pauses the timer and records its elapsed minutes. `session.log`
  with minutes records timed work. Either operation accepts an empty or partial values map.
  Supplied values must pass the metric's kind and range checks. Archived metrics are
  omitted from prompts and reject new values. Edits retain existing archived samples.
- To mark an item done, call `session.log` with its `item_id` and omit `minutes`.
  If that item has a running timer, the server stops it at the usual clamp point first
  and writes the tick in the same transaction. The tick receives the supplied metrics
  and note. The timer receives no new values. The timer keeps its original day and
  the tick uses the log's day. Both writes share one revision and `op_id`.
  `changed.sessions` returns the new tick first, followed by the stopped timer.
- `fulfilment.done_items` counts planned items with at least one live tick. Removing
  the last tick for an item with `session.remove` clears its done state for that day.
  Its timed sessions still count toward `actual_minutes` and `fulfilment_ratio`.
- An open timer has `running: true` until it is stopped. Its `stale` flag becomes
  true when the request time is strictly past its planning day's Sleep instant.
  Stopping a stale timer sets `ended_at` to that Sleep instant and records the
  clamped minutes (at most 1440). Finished sessions and ticks have `stale: false`.
  If status can read the timer but cannot resolve its Sleep instant, it keeps the
  running session, omits `stale`, and reports the failure in `planner_error`.
  Earlier v1 responses, including stored mutation replays, can also omit `stale`.

`now.next.at` in `day.get` and `next.at` in `status.get` are UTC instants. A
client computes a countdown as `next.at - Date.now()`; it does not add local
boundary maths.

`item.preview` is read-only and needs no `op_id`. It validates the draft like
`item.add` (`block` may not be `sleep`, a weekly draft needs weekdays) and
returns the first occurrence on or after `date`, the number of remaining
occurrences in that calendar month, and `schedule_text`. It never reads or
writes the store.

`item.move` accepts `after_id` or `before_id` and resolves them inside the
write transaction. Clients should send a neighbour id from the rendered row
list.

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
  write, even if the retry body differs. An `op_id` belongs to the method it
  was first used with: reusing it for another method is `validation`
  (`data.field = "op_id"`), and an empty `op_id` is rejected the same way. On first application the dispatcher
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
- `expected_version` applies per entity on every `*.edit` and `*.remove`,
  `item.move`, and optionally prayer marks. Appends (`*.add`, `session.start`,
  `session.stop`, `session.log`) need no precondition, only `op_id`. A stale
  version is `data.kind = "conflict"` and writes nothing.
- Every entity row carries its `version` (lists, day rows, and `changed`). `revision` orders pushes and de-duplicates them; it
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
           "scopes":["items","sessions"],"op_id":"…","actor":"widget"}}
```

`changed` is an invalidation, not a patch: refetch `status.get` or `day.get`.
`days` and `op_id` may be omitted; each scope is one of `items`, `sessions`,
`projects`, `tags`, `metrics`, `marks`, `config`. The daemon emits these from its `data_version` watcher; a
notification may cover more than one commit observed in the same poll, so
clients treat its `revision` as the newest state to fetch and de-duplicate by
revision.

The daemon also pushes `clock.boundary` when the current block or next boundary
changes, and once as an immediate snapshot after `subscribe` with the `clock`
topic:

```json
{"jsonrpc":"2.0","method":"clock.boundary",
 "params":{"day":"2031-05-14","block":"fajr",
           "now":"2031-05-14T04:30:00Z",
           "next":{"id":"dhuhr","name":"Dhuhr","time":"12:19",
                   "at":"2031-05-14T07:19:00Z","remaining_seconds":10140}}}
```

`now` and `next.at` are the authoritative instants. `remaining_seconds` is a
convenience snapshot at that instant; clients should count down from `next.at`
rather than assuming the notification arrives at `now`.

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
