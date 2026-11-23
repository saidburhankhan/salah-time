# TR brief (Codex): app Trends line chart and stale running card

You are worker TR for work item `tracking-ui` in /home/saidburxon/rust/salah (branch
`architecture`). You own `app/**` and `docs/work/tracking-ui/tr-progress.md`.

Read: `docs/AGENTS.md`, `plan.md` ("User decisions", "Phase 2" and its addenda, especially
"Addendum: Trends line chart", whose contract is locked), `c4-progress.md` (how Statistics, the
shared FilterBar, the Charts/Table/Notes tabs and the screenshot test were built),
`docs/work/architecture/protocol-v1.md` (`stats.series`), and the fixtures
`crates/salah-cli/tests/fixtures/rpc/stats_series_day.json` and `stats_series_split_item.json`.

Tasks:
1. The Trends chart on the Charts tab exactly as the addendum describes: series picker (measures,
   Split by Task/Project/Tag), Day/Week switch, lines with dots, legend chips toggling lines, gaps
   for missing values, shared axis for one unit, the default Split view (stacked small charts with
   a shared x axis) and a Relative view for mixed units, hover tooltip with every line's real
   value and sessions, Chart/Table toggle. It follows the shared filter bar. Muted palette, max 8
   lines. CHECKPOINT 1.
2. Running card for a stale timer (`stale: true`, which ran past Sleep): do not keep counting up
   (today it shows e.g. 600:01). Show the time stopped at the Sleep clamp (`max_minutes`) with one
   muted line "Ran past Sleep: stop it to save up to N min". CHECKPOINT 2.

The user's rules (strict): no bright white fills (theme tokens `selectedFill`/`selectedBorder`/
`selectedText`, muted surfaces in Main.qml); MM:SS timers with tabular numbers; `Text.PlainText`
on dynamic strings; QML files under ~800 lines. Checks: `app/bin/build`, `app/bin/test` (socket
tests skip in your sandbox; say which), `git diff --check`. Extend the screenshot test (Trends: Focus split by task over 28 days; Minutes + Focus in Split view;
Relative view; the stale running card), render with `SALAH_UI_SHOT_DIR=<dir> app/bin/test`, open
every PNG and fix clipping, overlap, white fills or unreadable lines before each checkpoint.

## Checkpoint protocol

Your sandbox cannot write `.git` or create Unix sockets. Never run `git commit`. At each green
checkpoint (all checks pass except tests that need Unix sockets) append to your progress file a
line `CHECKPOINT <slice>: <one-line commit message>` listing the files, then END your turn. The
orchestrator (Claude session `salah-orch`) runs the socket tests, reviews, commits, and resumes you.
Do not edit files outside your ownership. Never touch ~/.local/share/salah or the live daemon;
do not run ./scripts/install.
