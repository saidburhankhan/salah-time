# Salah

Salah is an offline prayer-time widget, planner and time tracker for Omarchy.
The day is divided by the five prayers (Fajr, Dhuhr, Asr, Maghrib and Isha),
and the configured sleep time ends it. Tasks are planned into the blocks between
the prayers, work is tracked as sessions, and the results feed statistics,
goals and a weekly review.

- **Bar widget**: the current block and the countdown to the next prayer or
  sleep; a popup with the day's timeline, the running session (pause, done,
  distractions, notes), quick add and task creation.
- **Desktop app** (`salah open`, or "Salah" in the launcher): Today, Backlog,
  Routines, Statistics (charts, a multi-measure Trends chart, tables and
  session notes), Insights (patterns, best and hardest sessions, plan
  adherence), Notes, Goals, Review and Settings.

In the app and the widget, click a note to edit it in place. Enter saves,
Shift+Enter starts a new line and Esc cancels; Esc also closes the widget one
layer at a time.

## Concepts

- **Tasks** are one-time (dated, or undated in the Backlog) or **routines**
  that repeat on chosen days. A routine can be ended on a day and resumed; its
  past days keep their history.
- **Projects** group tasks and can be split into **epics**; **tags** label
  tasks. Archiving a project pauses its goals, routines and open tasks until it
  is unarchived. Paused periods never count as missed, and statistics keep all
  of its work.
- **Sessions** are timed or logged work on a task, epic or project. A session
  can record metrics (focus, quality, distractions and custom metrics such as
  pages read), a main note and timed notes.
- **Goals** set a daily or weekly target in minutes or in a metric, for all work
  or for a project, epic, tag or task. Editing a target starts a new period and
  never rewrites the past.
- **Resources** are notes, links and reminders attached to tasks and epics.

## Command line

Quick add understands the prayer, repeat and length in one line:

```sh
salah item add --quick -- "read 20m after isha daily"
salah item parse --json -- "pushups at asr weekdays"   # preview without saving
```

Track work and its notes:

```sh
salah session start --item <id>
salah session note -- "stopped at page 12"
salah session stop
salah session log --item <id> --minutes 25 --value focus=4 --note "calm"
salah notes list --days 30 --min distractions_per_hour=4 --kind entry
```

Read the day and the reports:

```sh
salah day
salah stats --days 30
salah insights --json
salah goal list
salah review show
```

Every command has `--help`, and most accept `--json`. Ids may be shortened to
a unique prefix. Other subcommands manage projects, epics, tags, metrics,
resources, the backlog, routines and prayers (`salah --help`).

## Your data

All data lives in one SQLite database at
`${XDG_DATA_HOME:-~/.local/share}/salah/salah.db`. Set `SALAH_DATA` to use a
different path. Before a schema migration Salah keeps a backup next to it
(`salah.db.bak-v<N>-…`, the newest three).

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

## Daemon and socket transport

`salah serve` runs one long-lived, single-writer JSON-RPC 2.0 daemon and listens
only on `$XDG_RUNTIME_DIR/salah/salah.sock` (directory `0700`, socket `0600`).
Clients use NDJSON frames: `hello` first, then the protocol-v1 methods, plus
`subscribe` to receive `changed` invalidation pushes. `salah rpc` remains the
stdio transport and fallback; `salah rpc --socket` bridges stdin/stdout to a
running daemon.

The installer also writes user systemd units
(`~/.config/systemd/user/salah.socket` and `salah.service`) so a connect starts
the daemon on demand; `./scripts/uninstall` removes them. There is no TCP
listener and no authentication token: same-user access is enforced by file
permissions. See `docs/protocol-v1.md` for the full method
table and notification rules.

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
The plugin, desktop-app, and systemd-unit installation is ownership-checked:
the installer never overwrites files it did not install, and it refuses to
replace a plugin with the same name if that path belongs to something else or
if the previously installed copy has local modifications. For the real home it
also runs `systemctl --user daemon-reload` and enables `salah.socket`; failures
are reported without failing the install.

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
crates/salah-domain/   pure day, item, session and goal rules (no I/O)
crates/salah-prayer/   calculation, official timetable, location, schedule
crates/salah-store/    SQLite persistence and numbered migrations
crates/salah-core/     config and use cases: planner, sessions, stats, goals, insights, review
crates/salah-protocol/ JSON-RPC methods, dispatcher and notifications (protocol v1)
crates/salah-cli/      `salah` binary, daemon, integration tests, contract fixtures
app/                   Qt Quick desktop app (C++ models, QML pages)
plugin/local.salah/    Omarchy bar widget and popup
scripts/install        reversible local install (binary, app, plugin, systemd units)
scripts/uninstall      ownership- and content-checked uninstall
```
