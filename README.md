# Salah

Salah is a small, offline prayer-time widget for Omarchy. The bar always
shows the next prayer and the time remaining. Selecting it opens a compact
daily schedule with the location, calculation method, prayer times, and a
larger countdown.

The Rust binary owns calculation and settings. The Omarchy/Quickshell plugin
only renders the bar label and popup, so the same calculation can be tested
and queried from a terminal.

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
copies the plugin into `~/.config/omarchy/plugins/local.salah`. It refuses to
replace a plugin with the same name if that path belongs to something else or
if the previously installed copy has local modifications.

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

The popup reads `salah status --json`. Human-readable output is also useful
for checking the result independently:

```sh
salah status
salah status --json
salah config show
salah config set --help
```

The JSON output includes the location and method, every prayer for the active
date, the next prayer, its wall-clock time, and the remaining seconds. This
keeps UI code free of prayer-time mathematics.

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
src/                 Rust calculation, configuration, and CLI
tests/               calculation and CLI tests
plugin/local.salah/  Omarchy bar widget and popup
scripts/install      reversible local install
scripts/uninstall    ownership- and content-checked uninstall
```

