# Target front-end architecture

## Boundaries

Rust owns prayer/day boundaries, recurrence and next occurrence, item state transitions, timer accrual, ordering, validation, statistics aggregation, locale-neutral semantics, persistence, revisions, and conflict rules. A front end owns navigation, focus, open dialogs, pointer/keyboard gestures, animation, transient pending/error presentation, and locale-specific rendering. No UI should infer whether an item occurs, calculate a canonical move index, or reproduce a state transition.

Every response should use a versioned envelope:

```json
{"schema_version":1,"revision":184,"data":{},"capabilities":["events.v1"]}
```

Mutations carry `operation_id` and `base_revision` and return the canonical changed entity plus the new revision. A stale revision returns a structured conflict and current projection, never silent last-write-wins behavior.

## Desktop app

Suggested layout (names are illustrative):

```text
app/
  src/
    main.cpp
    client/          CoreClient, CliTransport, decoder, DTOs, errors
    models/          DayTimelineModel, RoutinesModel, StatsModel, SettingsModel
    controllers/     DayController, StatsController, OptimisticJournal
    qml/
      Main.qml
      pages/         TodayPage, RoutinesPage, StatisticsPage, SettingsPage
      timeline/      TimelineView, PrayerRow, ItemRow, MarkerRow, AddRow
      dialogs/       QuickAddDialog, ItemEditor, DateRangeDialog, HelpDialog
      controls/      AppButton, ChoiceChip, ErrorBanner, EmptyState
      js/            presentation-only formatting helpers
    i18n/
  tests/
    unit/            decoders, models, journal, controllers
    qml/             focus, keyboard, drag/drop, dialogs, accessibility
    fixtures/        versioned CLI JSON shared with Rust contract tests
```

`CoreClient` is the only process/socket boundary. Initially `CliTransport` executes fixed argv, serializes mutations, coalesces superseded reads, enforces timeouts/output bounds, and strictly decodes envelopes. It exposes typed async operations rather than command strings.

Use `QAbstractListModel` for timeline rows, routines, daily statistics, and per-item series. Give roles explicit types/meaning (`rowKind`, `itemId`, `title`, `pending`, `operationError`, etc.); update rows in place so delegates, focus, selection, and scroll survive. Small QObject view-models hold headers, totals, selection, loading, and errors. QML binds to these models and emits intents such as `toggleRequested(id)` or `moveRequested(id, beforeId, block)`.

Calendar widget layout and chart geometry may remain presentation logic. Date range normalization, occurrence previews, statistics selection, canonical ordering, timer totals, and validity belong in Rust. C++ controllers coordinate queries and optimistic presentation but must not reinterpret domain rules.

## Bar widget

Keep the required `BarWidget.qml` entry point, but make it a thin composition:

```text
plugin/local.salah/
  BarWidget.qml
  controller/        SalahController.qml, CliTransport.qml, Contract.js
  components/        BarLabel.qml, SalahPanel.qml, QuickAdd.qml, ErrorState.qml
  timeline/          Timeline.qml, PrayerRow.qml, ItemRow.qml, MarkerRow.qml
  i18n/
```

Within Quickshell constraints, one controller owns processes, the action queue, decoding, and the optimistic journal; components receive primitive properties/models and emit intents. Pure JS helpers are acceptable for envelope validation and presentation formatting, not recurrence, timer, or ordering policy.

Add a richer core projection such as `salah view widget --json` (or evolve `status --json`) that returns ordered display rows, localized-independent label keys/arguments, progress values, current/next markers, timer snapshot time, and allowed actions. QML should render that projection and update the countdown from an authoritative snapshot; it should not join planner items to blocks, sort positions, find next occurrences, or synthesize state transitions.

## Sharing without another duplicated framework

Share at the API boundary, not by trying to import desktop QML into Quickshell:

- Rust DTO definitions and checked JSON fixtures are the source of truth; generate C++ types/schema and validate the widget decoder against the same fixtures.
- Rust returns enum IDs plus stable presentation keys and interpolation arguments (`prayer.fajr`, `repeat.weekly`, `{minutes: 20}`), not final translated sentences.
- Maintain one canonical i18n key inventory and locale files, then generate/package the Qt and Quickshell formats; CI rejects missing or orphaned keys.
- Keep locale-sensitive date/number formatting in clients, driven by structured values. Core may return display-ready structural view models, but not English strings.
- If a tiny shared JS formatting module is practical in both runtimes, limit it to pure key/argument and duration formatting; never put domain policy there.

This balances consistency with accessibility: bots can use semantic DTOs, while visual clients can format for their locale and screen reader.

## From CLI to a local platform service

### Now: hardened CLI adapter

Keep fixed-argv CLI calls, but add versioned envelopes, operation IDs, revisions, strict decoding, output/time limits, canonical mutation responses, and a single `snapshot`/view command per surface. Each UI has one client/queue and coalesces reads. SQLite transactions remain authoritative, and compare-revision semantics make app/widget concurrency visible.

### Later: daemon and socket API

Introduce `salahd` only when multiple live clients or integrations justify it. It exclusively coordinates mutations and exposes a user-local Unix socket using JSON-RPC 2.0 (or an equivalently small framed protocol). Method families should be semantic—`day.get`, `item.add`, `item.move`, `item.complete`, `timer.start`, `stats.query`, `events.subscribe`—not shell command mirrors. Authenticate by user/socket permissions, bound messages, retain CLI as a thin client, and keep the Rust application layer transport-agnostic.

The app, widget, CLI, and local integration adapters connect to the same API. Telegram/Slack/notes connectors translate external identities/events into application commands; they do not access SQLite or duplicate scheduling rules. Network-facing bots should be separate least-privilege processes, not plugins loaded into the core daemon.

Subscriptions push `revision_changed`, entity changes, timer changes, prayer boundaries, and integration status. Clients stop 30/60-second data polling; local timers update only visual countdown text until a pushed authoritative event arrives.

## Optimistic UI protocol

1. Capture the authoritative base revision and enqueue an intent with a UUID.
2. Apply a small typed presentation patch and store its inverse; mark affected rows pending.
3. Send `{operation_id, base_revision, params}` in client order. Independent clients may act concurrently.
4. On success, replace affected data with the canonical response, advance revision, then rebase remaining patches.
5. On validation/transport failure, apply the inverse immediately, attach a row-level error/retry action, and rebase later patches.
6. On conflict, load the returned/current projection, replay still-valid intents, and ask the user only when intent cannot be preserved.
7. Treat pushed events bearing the same operation ID as acknowledgements, so an HTTP/socket response and event cannot apply twice.

Pending creation uses a client UUID that the core echoes and maps to the permanent ID. Deletes remain tombstoned/pending until acknowledged. Moves use neighbors/block rather than a stale numeric index where possible.

## Migration plan

Each numbered step should be a small reviewable commit with tests green.

1. **Freeze contract fixtures.** Add valid, malformed, old-version, and future-version envelopes for day/status/stats and every mutation; run them in Rust, C++, and widget decoder tests. This can proceed in parallel with UI component extraction because it adds new fixture/test files.
2. **Version and validate.** In Rust, add envelope/version/revision/operation IDs and canonical mutation responses. In the app, add DTO decoders behind the current `Backend`; in the widget, add `Contract.js` behind current rendering. Coordinate only on contract files/fixtures.
3. **Extract the desktop client.** Move QProcess lifecycle, coalescing, typed errors, and command construction into `client/` without changing screens. Add timeout, malformed-output, queue, and stale-revision tests.
4. **Introduce typed desktop models.** Build `DayTimelineModel` first and adapt Today; then routines/stats/settings one page at a time. Preserve existing QML behavior and delete matching `QVariantMap` paths only after each page passes tests.
5. **Split desktop QML.** Extract controls and delegates, then pages/dialogs. Do this after model interfaces settle; it can run in parallel with Rust recurrence/view-query work because it touches only `app/qml` and resources.
6. **Move domain projections to Rust.** Add occurrence preview, canonical move intent, timer snapshot, and widget/day view queries; replace QML date/repeat/order calculations one function at a time with fixture-backed output.
7. **Extract the widget controller/components.** Preserve `BarWidget.qml` as entry point, move process/controller logic first, then timeline rows and panel sections. Add pure controller tests and Quickshell interaction smoke tests.
8. **Add the optimistic journal.** Implement the same protocol semantics in the C++ controller and widget controller; test failure rollback, queued dependent actions, conflicts, external writes, and duplicate acknowledgements.
9. **Unify i18n generation and accessibility.** Establish canonical keys, migrate both catalogs incrementally, then add keyboard reorder, focus traversal, accessible names/roles, and table alternatives for charts.
10. **Package and observe.** Move the app to reproducible CMake/install rules, build/package it in CI, validate plugin/API compatibility, and record command latency/queue/conflict metrics locally without personal content.
11. **Add `salahd` behind `CoreClient` only after evidence.** Implement socket transport and event subscriptions, keep CLI fallback, migrate widget/app independently, then add integrations as separate adapters.

Parallel work should use new files/directories first. Rust work owns contract/application/view-query files; desktop work owns `app/client`, `app/models`, and new QML components; widget work owns new controller/component files. Schedule the small final edits to `Main.qml`, `BarWidget.qml`, resource manifests, and build files as explicit integration commits to avoid conflicts.

## Do not do yet

- Do not add a daemon merely to hide monolithic UI code; establish the application API and typed clients first.
- Do not expose SQLite, Rust persistence structs, or raw SQL to any front end or integration.
- Do not share a giant cross-runtime JavaScript domain library or make English display strings the contract.
- Do not adopt a general event bus, microservices, remote HTTP server, plugin SDK, or code generator before two real consumers require it.
- Do not rewrite both UIs at once; use adapters and vertical slices while preserving working interaction tests.
- Do not promise offline multi-device sync/CRDT semantics; revision conflicts and idempotent operations are sufficient for the local platform stage.
