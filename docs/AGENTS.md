# Agent guide

Salah is a small Rust prayer-time backend plus an Omarchy/Quickshell bar
plugin. Keep it focused: show the next prayer in the bar and reveal a compact
daily timetable when selected.

## Architecture

- `src/calculation.rs` contains deterministic offline astronomical maths.
- `src/location.rs` resolves location, preferring the saved Omarchy Weather
  coordinates before bounded HTTPS IP-location providers.
- `src/official.rs` downloads, validates, and caches the Muslim Board of
  Uzbekistan daily Tashkent timetable.
- `src/status.rs` selects the active schedule and produces the JSON consumed
  by QML.
- `src/config.rs` validates settings and replaces the config atomically.
- `plugin/local.salah/BarWidget.qml` owns presentation and process lifecycle;
  it must not contain prayer-time mathematics.
- `scripts/install` and `scripts/uninstall` are ownership-checked and must not
  overwrite or delete unrelated user files.

## Source precedence

For Tashkent with the `uzbekistan` method:

1. A valid Muslim Board schedule cached for the current local date.
2. Offline astronomical calculation.

For location:

1. Omarchy Weather's saved coordinates.
2. `geo.omarchy.org`.
3. `ipapi.co`.
4. The last valid saved configuration when online lookup fails.

Never label calculated values as official. A network or parsing failure must
leave existing configuration and cached schedules intact.

## Safety invariants

- Invoke fixed commands and URLs without a shell.
- Bound response sizes and timeouts, then validate UTF-8, JSON, coordinates,
  names, UTC offsets, dates, and ordered prayer times.
- Render dynamic QML strings with `Text.PlainText`.
- Keep automatic location throttled and safe across multiple monitors.
- Manual coordinates disable automatic location unless explicitly re-enabled.
- Do not edit `/usr/share/omarchy`; user integration belongs under
  `~/.config/omarchy`.
- Keep the plugin entry point named exactly `BarWidget.qml`. Quickshell rejects
  alternate case/type names, and a running shell may require
  `omarchy restart shell` after a structural plugin change.

## Development loop

Run the complete local verification:

```sh
./scripts/test
cargo clippy --offline --all-targets -- -D warnings
```

For a live check:

```sh
./scripts/install
salah location sync --json
salah official sync --json
salah status --json
omarchy restart shell
```

Confirm the popup identifies its active source. Test both network failure and
cached/offline operation. Do not rely only on the bar label.

## Configuration and state

- Config: `${XDG_CONFIG_HOME:-~/.config}/salah/config.json`
- Official cache: `${XDG_CACHE_HOME:-~/.cache}/salah/official.json`
- Installed binary: `~/.local/bin/salah`
- Installed plugin: `~/.config/omarchy/plugins/local.salah`

The Uzbekistan calculated preset is provisional because the Board has not
published the precise formula used by its timetable. Reference fixtures must
name their source and date.

