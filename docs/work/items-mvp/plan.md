# Plan: items-mvp

## Outcome

Salah becomes a minimal planner organised by prayer blocks. From the widget the
user adds an item to the current block, ticks it off, or tracks minutes/counts.
The `salah` desktop app (in this repo) shows the history by day and block and a
simple per-item total. All data lives in one local SQLite store that can be
exported and imported losslessly, and whose shape already permits tags,
projects, streaks, and cloud sync without rewriting stored data.

## Product principles

- The prayers are fixed points; items fill the blocks between them.
- Widget = now (current block). App = the past and planning.
- History is for reflection, not scoring. No streaks/points in the MVP.
- Every feature must help the user feel and use their blocks.

## Decisions

- One concept: an **item**. Repeat is `none` or `daily`. Measure is `check`,
  `minutes`, or `count`.
- An **entry** records what happened to an item on a day (done, minutes,
  count, or a timer session). Items are intentions; entries are history.
  Streaks, analytics, and charts are queries over entries.
- The **day** runs from Fajr to the sleep boundary. It is keyed by the local
  civil date of its Fajr (`DayKey`, `YYYY-MM-DD`). Between midnight and Fajr
  the active day is the previous date.
- A **block** is `(DayKey, BoundaryId)` where the boundary starts the block:
  `fajr`, `dhuhr`, `asr`, `maghrib`, `isha`. `sleep` starts the sleeping block,
  which takes no items. Sunrise is not a block boundary.
- Unfinished one-time items carry forward through the rest of their day and
  appear in the current block. When the day ends they become "not done" in
  history (derived, never rewritten). Tomorrow they can be re-planned.
- Timers stop automatically at the end of the block they started in. The
  stop is derived when reading and persisted on the next write.
- Ids are random UUIDv4 strings (v7 shares its leading timestamp characters, which made short ids ambiguous). Rows carry `created_at`, `updated_at` (UTC RFC3339)
  and `deleted_at` tombstones so deletions survive export/import and sync.
- Storage is SQLite at `${XDG_DATA_HOME:-~/.local/share}/salah/salah.db`,
  WAL mode, busy timeout, numbered migrations in `PRAGMA user_version`.
- Export is versioned JSON (`format: "salah-export"`, `version: 1`) including
  tombstones. Import is idempotent: upsert by id, newer `updated_at` wins.
- The widget and the app never touch the database; they call
  `salah … --json`. Status JSON gains `schema_version`.
- The CLI moves to `clap` so new subcommands stay correct.
- Future extensions are additive: tags = `item_tags` table, projects =
  nullable `items.project_id`, prayer tracking = built-in items.

## Data model (interface contract)

```text
items(id TEXT PK, title TEXT, block TEXT, repeat TEXT, measure TEXT,
      target INTEGER NULL, day TEXT NULL,          -- day: one-time items only
      created_at, updated_at, deleted_at NULL)
entries(id TEXT PK, item_id TEXT, day TEXT, block TEXT,
        kind TEXT,                 -- done | minutes | count | timer
        value INTEGER NULL,        -- minutes or count
        started_at NULL, ended_at NULL,          -- timer sessions
        created_at, updated_at, deleted_at NULL)
```

## Non-goals

- Tags, projects, streaks, prayer tracking, reminders, cloud sync.
- Weekly/custom repeat rules.

## Slices

- [ ] S1 — CLI migration to clap with unchanged behaviour (Codex; `src/main.rs`, `tests/cli.rs`)
- [ ] S2 — `schedule` extraction, typed `BoundaryId`, `DayKey`, block key (lead; `src/day.rs`, `src/status.rs`, new `src/schedule.rs`)
- [ ] S3 — item/entry domain and SQLite store with migrations (lead; `src/item.rs`, `src/store.rs`)
- [ ] S4 — export/import and round-trip tests (worker; `src/export.rs`, `tests/export.rs`)
- [ ] S5 — `salah item …`, `salah export|import` commands, status `items` block
- [ ] S6 — widget: current block items, tick, quick add, open count, sleep time
- [ ] S7 — `app/` Qt Quick desktop app: day/block history and per-item totals
- [ ] S8 — integration, install, live verification

## Acceptance checks

- [ ] An item added after midnight but before bedtime belongs to the previous day.
- [ ] An unfinished one-time item appears in later blocks of its day, then is "not done" in history.
- [ ] A timer left running stops at the end of its block.
- [ ] Export → import into an empty store reproduces the data, including deletions; importing twice changes nothing.
- [ ] A store from an older migration upgrades in place.
- [ ] Existing `status --json` fields are unchanged.
- [ ] `./scripts/test`, strict Clippy, `git diff --check` pass.
- [ ] Widget and app installed and inspected live.
