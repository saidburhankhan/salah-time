# W2 progress — tracking-model S5

Last commit: backend/orchestrator HEAD `dbbf021`; W2 checkpoint is intentionally uncommitted for orchestrator review and commit.

Current step: CHECKPOINT — restored frontend coverage is adapted to the final RPC fixtures and the app/widget use the tracking protocol.

Next step: orchestrator verification and commit.

## Implemented

- Restored `app/tests/salah_app_tests.cpp` from HEAD before adapting it. It now reads real envelopes in `crates/salah-cli/tests/fixtures/rpc/`; `app/tests/fixtures/` is gone.
- Mutations treat `changed.items` as invalidations and refetch `day.get`; no frontend patching assumes a day-row shape.
- App Today uses project/planned/session and capacity/planned/actual data; item editing uses project and planned minutes.
- Settings exposes project, tag, and metric RPC CRUD flows, including nested project metric definitions and the built-in archive guard.
- Stats consumes the minutes pseudo-series, nullable daily fulfilment, project/block breakdowns, and 7/28/90 ranges.
- Widget uses `session.start`, `session.stop`, and `session.log`, top-level `running`, nested project metrics for +Log, and reopens the prompt panel with `error.data.missing` metrics highlighted.
- CLI mutation fallbacks decode the same mutation-result shape as socket responses.

## Test-suite review

- Current app suite: 48 test functions; HEAD: 48. Socket/reconnect/conflict/notification, queue, model-diff, real-QML drag/drop, overlay, theme, and translation coverage remains.
- Removed test: `adjustmentArgumentsAndClamp` — it covered the removed legacy `item log`/entry minute/count adjustment controls. The replacement protocol records through `session.log` and prompted metric values.
- Renamed/adapted, not removed: `coreClientDecodesTimerStopArray` → `coreClientDecodesSessionStopMutation`; `timerStartMarksOnlyTheTouchedRowPending` → `sessionStartMarksOnlyTheTouchedRowPending`.
- Added `trackingSurfacesFollowCanonicalFixtures` for missing-metric prompts, nested project metrics, settings CRUD routing, and the minutes pseudo-series/project/block stats shape.
- Golden decoder coverage consumes real RPC fixtures, including session/settings mutations, day capacity data, and stats series/breakdowns.
- All 16 AF_UNIX/FakeDaemon tests now use canonical `<id:4>`/`<id:5>` rows and mutation arrays. Successful mutation fakes update their authoritative `day.get` response because `changed.items` is only an ItemView invalidation.
- `coreProjectionTextDrivesApp` now exercises planned/actual/session/project projection, the canonical stats minutes series, and the real preview fixture; its legacy row and `daily`/`time_ticks` mutations were removed.
- Socket run #2 fixes: day-row `sessions` is treated as a SessionView array; mutation refresh is deferred until SocketTransport has removed the completed request; drag gaps prefer `before_id` when a following sibling exists.
- Plugin Node checks were not removed or edited: `plugin/local.salah/tests/drag_logic_test.js` remains unchanged from HEAD with 29 checks.

## Verification

- `app/bin/build`: PASS.
- `app/bin/test`: PASS — 29 passed, 0 failed, 16 skipped. Skips are retained AF_UNIX fake-daemon tests; this managed sandbox denies Unix socket bind.
- `node plugin/local.salah/tests/drag_logic_test.js`: PASS — 29 checks.
- `omarchy plugin validate plugin/local.salah`: PASS (exit 0, no output).
- `git diff --check`: PASS.

## Contract mismatches

- Current `day_tracking.json` fulfilment has `unplanned_minutes` but omits the newly locked nullable `fulfilment_ratio`. The fixture still needs the backend regeneration mentioned by the orchestrator.

## Remaining gaps

- Re-run retained socket and real-QML socket-backed tests outside this restricted sandbox; the previous orchestrator run exposed the now-fixed canonical-row and null-array failures.
- Re-sync the day fixture assertion once backend regeneration adds `fulfilment_ratio`; no local substitute fixture remains.
