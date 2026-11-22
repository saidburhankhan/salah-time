# W2 progress — tracking-model S5

Last commit: none from W2 — commit blocked because this session's `.git` is read-only (`fatal: Unable to create .git/index.lock: Read-only file system`). The verified changes remain in the working tree for the orchestrator to commit.

Current step: app and widget are implemented against the locked tracking-model contract; local frontend verification is green.

Next step: orchestrator integration with W1, then switch app tests from `app/tests/fixtures/` when W1 regenerates the canonical RPC fixtures.

## Implemented

- Widget uses `session.start`, `session.stop`, and `session.log`; it shows the running project's colour/name and elapsed time, prompts scale/count/check metrics plus note inline, supports project/minutes quick logging, and logs ticks without minutes.
- Today exposes day fulfilment, block capacity/planned/actual bars, project colour, planned minutes, actual minutes, and session counts.
- Item editing uses a project picker and planned minutes.
- Settings provides Projects, Tags, and Metrics lists/add/archive flows; built-in metrics cannot be archived in the UI.
- Stats provides 7/28/90 ranges, filter choices, KPI tiles, metric series, minutes against capacity, and project/block breakdowns.
- Contract fixtures live in `app/tests/fixtures/` until the backend fixtures are regenerated.

## Verification

- `app/bin/build`: PASS
- `app/bin/test`: suite restored and builds; 30 non-socket tests pass after contract adaptation. The remaining 17 retained socket-backed tests cannot start their fake daemon in this managed sandbox because Unix-domain `bind` is denied (`QLocalServer::listen: Unknown error 1`). They were not skipped or deleted and need one run in the orchestrator's normal checkout environment.
- `node plugin/local.salah/tests/drag_logic_test.js`: PASS — 29 checks
- `omarchy plugin validate plugin/local.salah`: PASS (exit 0, no output)
- `git diff --check -- app plugin docs/work/tracking-model/w2-progress.md`: PASS

## Contract mismatches

- The currently landed backend RPC fixtures (`crates/salah-cli/tests/fixtures/rpc/day.json` and `status.json`) are still the legacy shape: day blocks omit capacity/planned/actual and fulfilment, while status exposes `planner.timer` and omits top-level `running` and `prompt`. They therefore cannot yet replace the app-owned locked-contract fixtures.
- The locked `status.get` result has no project catalogue, but the locked widget UX requires a project picker for `+ Log`. The widget calls the separately locked `projects.list` method when connected. No interface change is required, but the dependency should be explicit in protocol documentation/fixtures.
- The mutation `changed` object shape for new entities/sessions is not spelled out as completely as the old `items`/`entries`/`marks` shape. Frontends refresh authoritative reads after mutations rather than inferring undocumented projections.

## Remaining gaps

- Reconcile exact DTO field presence and mutation fixtures when W1's regenerated fixtures land.
- Live daemon/UI exercise is left to orchestrator S6; no install or shell restart was run.

## Test-suite review

The HEAD app suite was restored. Existing socket/reconnect, model-diff, real-QML drag/drop, queue, theme, translation, and overlay tests remain. The plugin Node suite was not changed or removed (`plugin/local.salah/tests/drag_logic_test.js`, 29 checks).

Removed tests:

- `coreClientDecodesTimerStopArray` — removed because `timer.stop` and its legacy array-of-entry result were explicitly removed; `session.stop` is covered through the new RPC/session tests.
- `adjustmentArgumentsAndClamp` — removed because direct legacy `entry.log` minute/count adjustments and the extra measurement controls were explicitly removed. Session logging and locked metric prompt kinds are covered instead.

Added/reworked coverage includes `sessionStartSendsOnlyTheTouchedItem`, `sessionPanelCoversLockedPromptKinds`, `capacityBarRolesRoundTripThroughModel`, `settingsCrudMethodsUseLockedRpcNames`, and `statsFixtureCarriesSeriesAndBreakdowns`.

Checkpoint test inventory: 56 test/helper functions in the current file versus 54 in HEAD. The real-QML drag/drop tests from `c673a99`, socket handshake/reconnect/conflict/notification tests, typed model diff tests, queue tests, theme/translation tests, and overlay tests remain present.
