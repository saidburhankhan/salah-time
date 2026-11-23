# C9 brief (Codex): app pages for insights, goals and weekly review

You are worker C9 for work item `tracking-ui` phase 4 in /home/saidburxon/rust/salah (branch
`architecture`). You own `app/**` and `docs/work/tracking-ui/c9-progress.md`.

Read: `docs/AGENTS.md`, `plan.md` ("User decisions", "Phase 2", "Phase 3", "Phase 4"), the C4/C7
progress files, `docs/work/architecture/protocol-v1.md`, and the phase 4 fixtures in
`crates/salah-cli/tests/fixtures/rpc/` (`insights.json`, `goals_*.json`, `goal_*.json`,
`review_*.json`, `status_goals.json`).

Tasks:
1. Insights page as plan.md describes: block / hour / weekday groups as compact bar rows with
   minutes, focus and distractions per hour; the distractions-per-hour line; Best and Hardest
   sessions with notes, opening the session; the On-plan rate per task and the planned × actual
   block table (plan.md "Addendum: sessions across prayer blocks"). Shared Stats filter bar. CHECKPOINT 1.
2. Goals: a Goals section in the project, epic and item editors (add: period, minutes or metric,
   direction, target; edit target; end), an overall Goals page with progress bars and per-goal
   history table. CHECKPOINT 2.
3. Weekly review page: week picker (default last full week), the computed summary, the optional
   reflection with Save. CHECKPOINT 3.

The user's rules (strict): everything optional; no bright white fills (theme tokens
`selectedFill`/`selectedBorder`/`selectedText` and the muted surfaces in Main.qml); MM:SS timers;
`Text.PlainText` for dynamic strings; small samples shown as counts, never hidden.

Checks: `app/bin/build`, `app/bin/test`, `git diff --check`. Extend the screenshot test and
render every page you change with `SALAH_UI_SHOT_DIR=<dir> app/bin/test`; open each PNG and fix
clipping, overlap, white fills, misalignment before each checkpoint.

## Checkpoint protocol

Your sandbox cannot write `.git` or create Unix sockets. Never run `git commit`. At each green
checkpoint (all checks pass except tests that need Unix sockets) append to your progress file a
line `CHECKPOINT <slice>: <one-line commit message>` listing the files, then END your turn. The
orchestrator (Claude session `salah-orch`) runs the socket tests, reviews, commits, and resumes you.
Do not edit files outside your ownership. Never touch ~/.local/share/salah or the live daemon;
do not run ./scripts/install.
