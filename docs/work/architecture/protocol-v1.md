# Protocol v1: JSON-RPC 2.0 over NDJSON

This is the transport-independent contract introduced in S3. It is served by
`salah rpc` over stdin/stdout today and by the S4 Unix-socket daemon without
changing a single method or DTO. The human `salah … --json` contract stays
frozen in [`contract-v1.md`](contract-v1.md); it is not the RPC contract.

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
`events.v1` is reserved for S4's `changed` push.

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
  write, even if the retry body differs. The planner records `(op_id → result,
  revision)` in `applied_ops` in the same transaction as the mutation.
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

Notifications have no `id`:

```json
{"jsonrpc":"2.0","method":"changed",
 "params":{"revision":185,"days":["2031-05-14"],
           "scopes":["items","entries"],"op_id":"…","actor":"widget"}}
```

`changed` is an invalidation, not a patch: refetch `status.get` or `day.get`.
`days` and `op_id` may be omitted; `scopes` is one of `items`, `entries`,
`marks`, `config`. S3 defines these types; S4 emits them from the daemon
together with `clock.boundary`.

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
