# Agent guide

Lead and orchestrator sessions must begin with `docs/START.md`. Workers with a
narrow assigned brief use this guide plus that brief and do not load unrelated
work-item history.

Salah is a small Rust prayer-time backend plus an Omarchy/Quickshell bar
plugin. Keep it focused: show the next prayer in the bar and reveal a compact
daily timetable when selected.

## Architecture

- `crates/salah-domain/` holds pure planning types and rules, with no I/O:
  day boundaries and timelines, items/entries, errors, and clock/id traits.
- `crates/salah-prayer/` contains deterministic offline astronomical maths
  (`calculation.rs`), the Muslim Board timetable parser/client/cache
  (`official.rs`), location discovery (`location.rs`), and schedule/block
  resolution (`schedule.rs`).
- `crates/salah-store/` is the SQLite store and its numbered migrations.
- `crates/salah-core/` is the application layer: validated config and its XDG
  repository, planner, sessions, stats, goals, insights, reviews, quick add,
  status, and location sync policy.
- `crates/salah-protocol/` holds the JSON-RPC protocol v1: method table,
  dispatcher, params/DTOs and `changed` notifications (`docs/protocol-v1.md`).
- `crates/salah-cli/` builds the `salah` binary and owns its integration tests
  and contract fixtures. It also runs the
  single-writer daemon `salah serve` on `$XDG_RUNTIME_DIR/salah/salah.sock`
  (socket-activated user units `salah.socket`/`salah.service`) and the stdio
  transport `salah rpc`.
- `app/` is the Qt Quick desktop app (C++ client, typed models, QML pages);
  `app/bin/test` runs its tests, including socket tests a sandbox may skip.
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

