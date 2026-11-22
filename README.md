# Salah

Salah is a small, offline prayer-time widget and planner for Omarchy. The day
is divided by the five prayers (Fajr, Dhuhr, Asr, Maghrib, and Isha), and
items, either one-time tasks or routines, are planned in the blocks between
them. The bar widget shows now: the current block and the next boundary. The
desktop app (`salah open`, or "Salah" in the launcher) shows the day, routines,
and statistics. The configured sleep time ends the day.

## Planning

Quick add understands the prayer, repeat, and measure in one line:

```sh
salah item add --quick -- "read 20m after isha daily"
salah item add --quick -- "pushups x30 at asr weekdays"
salah item add --quick -- "call mum 15 min on sundays"
```

Use `salah item parse --json -- "read 20m after isha daily"` to preview the
result without saving it.

The core item commands are:

```sh
salah item add --block fajr -- "Read Quran"
salah item list
salah item done <id>
salah item undo <id>
salah item log <id> 20
salah item start <id>
salah item stop
salah item move <id> --block asr --index 0
salah item edit <id> --title "Read Quran" --target 30
salah item remove <id>
```

An item is measured by check, minutes, or count, and repeats once, daily, or on
chosen weekdays. Item ids may be shortened to a unique prefix. `salah day`
shows one day's prayers and items by block, `salah prayer mark fajr` records a
prayer, and `salah stats --days 30` shows per-item and per-prayer totals.

Unfinished one-time items carry forward through the rest of their day and
appear in the current block. When the day ends they are history as missed. A
running timer stops automatically at the end of the block where it started.

Sleep ends the day. Set the local 24-hour boundary with:

```sh
salah config set --sleep-time 23:00
```

## Your data

Planner data lives in one SQLite database at
`${XDG_DATA_HOME:-~/.local/share}/salah/salah.db`. Set `SALAH_DATA` to use a
different path.

Export writes versioned JSON, including deletions, and import is idempotent:
importing the same file again changes nothing, and a newer `updated_at` wins.
Export refuses to replace an existing file unless `--force` is given.

```sh
salah export --output file.json
salah import file.json
```

## Build and test

Requirements: Rust/Cargo and a current Omarchy installation.

```sh
cargo build
cargo test
cargo run -- status
cargo run -- status --json
```

Run the complete offline test, plugin validation, and disposable install /
uninstall safety checks with:

```sh
./scripts/test
```

The default profile is Tashkent, Uzbekistan. Inspect the active settings and
available command options before changing them:

```sh
cargo run -- config show
cargo run -- config set --help
cargo run -- --help
```

## Automatic location

Salah refreshes its location on startup and at most once every six hours.
It first reuses the location selected in Omarchy Weather, which keeps the
lookup local. If Weather has no valid coordinates, it tries
`geo.omarchy.org` and then `ipapi.co`; those services receive the public IP
address and return an approximate city-level location. A VPN can change the
result. A failed lookup never replaces the last valid configuration.

```sh
salah location show
salah location sync --json              # force a refresh
salah location sync --if-needed --json  # honor the six-hour cache
```

Entering coordinates manually disables automatic location unless
`--auto-location true` is supplied explicitly.

For Tashkent with the Uzbekistan method, each refresh also downloads the
current daily timetable from the Muslim Board of Uzbekistan. The validated
schedule is cached in `~/.cache/salah/official.json`; `salah status` uses it
for that date and clearly reports `source: "official"`. If the site is
unavailable or the response cannot be validated, Salah retains the cached
schedule or falls back to the offline calculation. Force both updates with
the Refresh button in the popup, or update the timetable alone with:

```sh
salah official sync --json
```

Configuration is stored in the user's local configuration directory rather
than in this repository. The command-line interface validates coordinates,
UTC offsets, and calculation-method names before saving them.

## Install in Omarchy

The installer builds a release binary, copies it to `~/.local/bin/salah`, and
copies the plugin into `~/.config/omarchy/plugins/local.salah`. When `qmake6`
is present it also builds the desktop app, installs `~/.local/bin/salah-app`,
and adds a "Salah" launcher entry and icon; without `qmake6` it skips the app.
The plugin and desktop-app installation is ownership-checked: the installer
never overwrites files it did not install, and it refuses to replace a plugin
with the same name if that path belongs to something else or if the previously
installed copy has local modifications.

```sh
./scripts/install
omarchy plugin enable local.salah --after omarchy.clock
```

Omarchy reloads user plugin changes automatically. If the widget does not
appear after enabling it, rescan once:

```sh
omarchy-shell shell rescanPlugins
```

To remove the binary and plugin link while keeping personal settings:

```sh
./scripts/uninstall
```

## CLI contract

The bar widget and desktop app read JSON from the CLI. Human-readable output is
also useful for checking the result independently:

```sh
salah status
salah status --json
salah config show
salah config set --help
```

`salah status --json` includes the location and method, every prayer, the
ordered prayer/sleep boundaries, the current block, and the next boundary with
its wall-clock time and remaining seconds. It also embeds today's planner
items as `planner`; `salah day --json` returns the full timeline for one day.
This keeps UI code free of scheduling mathematics.

## Calculation notice

Calculated prayer times are estimates derived from the selected coordinates,
UTC offset, solar-angle preset, and Asr method. Local mosque or official
religious-authority schedules can include regional corrections that an
astronomical preset does not capture. Confirm the active preset and compare
the result with the official schedule for your location before relying on it.

The Uzbekistan preset must be treated as configurable until its parameters
and adjustments have been checked against an authoritative current source.
Do not present an unverified preset as an exact official timetable.

## Development layout

```text
crates/salah-domain/   pure day/item rules, errors, clock/id traits (no I/O)
crates/salah-prayer/   calculation, official timetable, location, schedule
crates/salah-store/    SQLite persistence and numbered migrations
crates/salah-core/     config, planner use cases, quick add, export, status
crates/salah-cli/      `salah` binary, integration tests, contract fixtures
app/                   Qt Quick desktop app
plugin/local.salah/    Omarchy bar widget and popup
scripts/install        reversible local install
scripts/uninstall      ownership- and content-checked uninstall
```
