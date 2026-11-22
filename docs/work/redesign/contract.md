# CLI contract: redesign

The app and the widget use only these commands. Each runs as a fixed
argument list (no shell); free text always follows `--`. Errors exit
non-zero with `salah: <message>` on stderr. Dates are `YYYY-MM-DD`, times
`HH:MM` (local), instants RFC 3339 UTC.

Enumerations:

- prayer / block: `fajr`, `dhuhr`, `asr`, `maghrib`, `isha`
- boundary id (adds): `sleep`
- repeat: `none`, `daily`, `weekly` (`weekly` always carries `weekdays`)
- weekday: `mon` `tue` `wed` `thu` `fri` `sat` `sun`
- measure: `check`, `minutes`, `count`
- state: `open`, `done`, `missed`

## DayItem

```json
{ "id": "4058ef7b-…", "title": "Read Quran", "block": "fajr",
  "repeat": "weekly", "weekdays": ["mon","tue","wed","thu","fri"],
  "measure": "minutes", "target": 30, "position": 0,
  "state": "open", "minutes": 20, "count": 0,
  "timer_started_at": null }
```

`weekdays` is `null` unless `repeat` is `weekly`. Items in a block are
ordered by `position`.

## Day plan — `salah day [--date D] --json`

```json
{ "day": "2026-11-22", "is_today": true,
  "now": { "block": "dhuhr", "sleeping": false,
           "next": { "id": "asr", "name": "Asr", "time": "15:17",
                     "remaining_seconds": 7800 } },
  "prayers": [
    { "id": "fajr", "name": "Fajr", "time": "05:56",
      "passed": true, "current": false, "marked": true },
    … dhuhr, asr, maghrib, isha (always 5, in order)
  ],
  "sunrise": "07:14",
  "sleep": { "time": "23:00", "passed": false },
  "blocks": [
    { "start": "fajr", "end": "dhuhr", "items": [DayItem, …] },
    … dhuhr→asr, asr→maghrib, maghrib→isha, isha→sleep (always 5)
  ],
  "open": 2 }
```

`now` is `null` unless the day is the current day. While sleeping,
`now.block` is `null`, `sleeping` is `true`, and `next` is Fajr. Default
date: the current day (Fajr→Sleep). Past days: every prayer `passed`.

## Today — `salah item list --json`

Unchanged from the MVP, plus `"prayer_marks": ["fajr", …]` for the current
day. `salah status --json` embeds it as `planner`.

## Quick add

- `salah item parse --json -- TEXT` → draft, nothing saved:
  `{ "title": "Read", "block": "isha" | null, "repeat": "daily",
     "weekdays": null, "measure": "minutes", "target": 20 }`
- `salah item add --json --quick -- TEXT` → saves the parsed draft; explicit
  flags (below) override parsed values. Returns the Item.

Grammar (case-insensitive, any order): `after|at <prayer>`,
`before <prayer>` (the block before), `<N>m|min|h|1h30m` (minutes target),
`x<N>|<N> times` (count target), `daily|every day`, `weekdays`,
`every mon,wed` / `on mondays`. The rest is the title.

## Items

- `salah item add --json [--block B] [--repeat none|daily|weekdays|weekly]
  [--days mon,tue,…] [--measure M] [--target N] [--date D] [--quick] -- TITLE`
  - `weekdays` = `weekly` with mon–fri. `--days` requires `weekly`.
  - `--date` plans a one-time item on that day (default: planning day).
  - `--daily` remains an alias of `--repeat daily`.
- `salah item edit --json ID [--title T] [--block B] [--repeat R] [--days …]
  [--measure M] [--target N | --no-target]`
- `salah item move --json ID --block B --index N` → moves the item to block B
  at position N (0-based) and renumbers that block. Returns the Item.
- `salah item done|undo --json ID [--date D]`, `salah item log --json ID VALUE
  [--date D]` — `--date` records on that day (default: as before).
- `salah item start|stop`, `salah item remove --json ID` — unchanged.

Item (from add/edit/move): DayItem fields without `state`, `minutes`,
`count`, `timer_started_at`; with `day`, `created_at`, `updated_at`,
`deleted_at`.

## Prayers

- `salah prayer mark --json PRAYER [--date D]`
- `salah prayer unmark --json PRAYER [--date D]`

Returns `{ "id": "2026-11-22:fajr", "day": "2026-11-22", "prayer": "fajr",
"marked_at": "…", "created_at": "…", "updated_at": "…", "deleted_at": null }`.
Marking twice is harmless. Default date: the current day.

## Statistics — `salah stats [--from D --to D | --days N] --json`

Default: the 30 days ending on the current day.

```json
{ "from": "2026-10-24", "to": "2026-11-22", "days": 30,
  "items": [
    { "id": "…", "title": "Read Quran", "block": "fajr", "repeat": "daily",
      "weekdays": null, "measure": "minutes", "target": 30,
      "planned_days": 12, "done_days": 9, "minutes": 310, "count": 0,
      "series": [ { "day": "2026-10-24", "planned": false, "done": false,
                    "minutes": 0, "count": 0 }, … one per day ] }
  ],
  "prayers": [ { "prayer": "fajr", "marked_days": 20, "days": 30 }, … 5 ],
  "marked_total": 96, "prayer_slots": 150 }
```

`items` lists every live item planned at least once in the range. Routines
are the entries with `repeat` ≠ `none`.

## Settings

`salah config show` (JSON) and `salah config set --sleep-time HH:MM` etc.,
unchanged.
