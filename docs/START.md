# Start here

Every lead or orchestrator session starts with this file. A worker that already
received a narrow brief skips this file and follows that brief plus
`docs/AGENTS.md`.

The orchestrator owns scope, file ownership, integration, verification, and
the final user report. Workers own only the slice assigned to them.

## First five minutes

1. Read `docs/AGENTS.md` completely.
2. Run `git status --short --branch` and inspect active work below.
3. Read only the selected work item's `state.md` and `plan.md`.
4. Confirm the requested outcome and the next verifiable step in one sentence.
5. Decide whether parallel workers will reduce latency without creating file
   conflicts. Do not delegate merely to create activity.

## Active work

| Work item | Status | Next | Updated |
|---|---|---|---|
| `items-mvp` | Installed | Superseded by `redesign` (same branch) | 2026-11-22 |
| `redesign` | Installed | Finish A9 app fixes; then superseded by `architecture` | 2026-11-22 |
| `tracking-model` | Design | Design app, widget, and stats on schema v1 | 2026-11-22 |

Completed work may be moved from `docs/work/` to `docs/archive/`. Do not load
archived work unless the current task explicitly depends on it.

## Starting new work

1. Choose a short kebab-case name, such as `official-regions`.
2. Copy `docs/work/_template/` to `docs/work/<name>/`.
3. Put the user's outcome, decisions, non-goals, and acceptance checks in
   `plan.md`.
4. Record the current branch, dirty files, current behavior, and immediate
   next step in `state.md`.
5. Replace the `_None_` row above or add a row for the new work item.
6. Tell the user: `New work <name>: <outcome>. First: <step>.`

If the request is a tiny self-contained fix that will finish in one session,
the orchestrator may skip creating a work item. It must still inspect the
worktree, protect unrelated changes, test proportionally, and report exactly
what changed.

## Continuing work

1. Read only `docs/work/<name>/state.md` and `plan.md` after this file and
   `docs/AGENTS.md`.
2. Compare the recorded branch and dirty files with Git. Git is authoritative;
   correct stale state before continuing.
3. Resume the first incomplete acceptance check or the explicit `Next` item.
4. Do not reopen old investigations unless current evidence invalidates their
   recorded conclusion.

## Delegating to workers

Delegate independent slices with explicit ownership. A good worker brief
contains:

- the exact outcome;
- files or directories the worker owns;
- files it must not edit;
- interfaces it must preserve or produce;
- required tests and commands;
- safety constraints from `docs/AGENTS.md`;
- the expected final report: findings, edits, verification, and remaining
  risks.

Example split for a substantial feature:

| Worker | Owns | Responsibility |
|---|---|---|
| Calculation | `crates/salah-prayer/src/calculation.rs`, focused tests | Maths, methods, reference fixtures |
| Data/source | `crates/salah-prayer/src/location.rs`, `crates/salah-prayer/src/official.rs` | Network validation, cache, fallback |
| CLI/status | `crates/salah-cli/src/main.rs`, `crates/salah-core/src/config.rs`, `crates/salah-core/src/status.rs` | Contract, persistence, rollover |
| UI/package | `plugin/`, `scripts/`, user docs | QML lifecycle, installation, presentation |

Do not assign the same file to multiple workers. If two slices require one
shared interface, the orchestrator defines the interface first and gives each
worker a different side. Workers communicate discoveries early; they do not
silently broaden scope.

Use a read-only reviewer after implementation when correctness, network
security, religious timetable accuracy, installer safety, or live desktop
behavior is involved. A reviewer should identify concrete blockers and test
gaps, not rewrite another worker's slice concurrently.

## Orchestrator integration loop

1. Inspect every worker's actual diff; do not trust summaries alone.
2. Resolve interface mismatches and remove duplicated logic.
3. Run focused tests for each slice.
4. Run the full repository checks:

   ```sh
   ./scripts/test
   cargo clippy --offline --all-targets -- -D warnings
   git diff --check
   ```

5. For plugin changes, install and inspect the live path:

   ```sh
   ./scripts/install
   omarchy plugin validate ~/.config/omarchy/plugins/local.salah
   salah status --json
   ```

6. Structural QML changes may require the user to run
   `omarchy restart shell` from the desktop session. Keep the entry point named
   exactly `BarWidget.qml`.
7. Check `journalctl --user` for new Salah/QML warnings before declaring the
   UI complete.

## When blocked

Exhaust safe local evidence first: source, tests, installed files, CLI output,
and user-session logs. Then report:

- the exact blocked operation;
- the evidence and error;
- what remains safe and working;
- the smallest user action or external change needed.

Never hide a calculated timetable behind an `official` label. Never replace a
valid config/cache after an online failure. Never modify `/usr/share/omarchy`.

## Before stopping

For tracked work:

1. Rewrite `state.md` so `Now` and `Next` describe the present, not the session
   history.
2. Update acceptance-check statuses and user decisions in `plan.md`.
3. Update the Active work row above.
4. Record the exact verification commands and their result.
5. If complete, move the work directory to `docs/archive/` and remove its
   active row.

For every task, leave the repository in a comprehensible state and tell the
user what is complete, what was verified, and what remains.

