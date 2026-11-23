# C8 brief (Codex): backend for insights, goals and weekly review

You are worker C8 for work item `tracking-ui` phase 4 in /home/saidburxon/rust/salah (branch
`architecture`). You own `crates/**`, `docs/work/architecture/protocol-v1.md` and
`docs/work/tracking-ui/c8-progress.md`. Other workers may edit other directories concurrently.

Read: `docs/AGENTS.md`, `docs/work/tracking-ui/data-model.md` (locked; "005" is yours and wins
over plan.md on storage), `plan.md` ("User decisions", "Phase 2", "Phase 3" incl. "Epics", and
"Phase 4", whose contract is locked), the c2/c5 progress files, and the code you own.

Tasks:
0a. The shared `block_minutes` rule from plan.md "Addendum: sessions across prayer blocks"
   (day view block bars, `stats.table` group "block", `stats.series` split "block") with its tests and
   `stats_table_block.json`. CHECKPOINT 0a.
0. Migration 005 exactly as data-model.md specifies, with its migration test on realistic rows
   (follow the existing migration tests). 005 includes `item_tags` (plan.md "Addendum: tags on tasks").
   CHECKPOINT 0 alone, for the independent review.
0c. Tags on tasks: item views `tag_ids`, `item.add` defaults from the project, `item.edit`,
   `tag.remove`, the session tag set in `stats.*`/`sessions.list`/`backlog.list` filters and the
   "tag" group, fixtures `item_add_tags.json`, `stats_table_tag_items.json`, CLI
   `salah item add/edit --tag`. CHECKPOINT 0c.
1. Goals: domain types, store queries, `salah-core` module `goals` (progress from sessions for
   minutes and for metrics using each metric's rollup and the minutes weighting stats uses),
   `goals.list`, `goal.add`, `goal.edit` (successor rule), `goal.end`, `goal.remove`,
   `goals.history`, `status.get.goals`, `changed` entity `goals`, ending goals when their
   scope is removed. Tests for day/week periods across the Sleep boundary and Monday, at_most,
   metric goals, edits mid-period, and "No project" work counting only toward all-work goals.
   CHECKPOINT 1.
2. `insights.get` in a `salah-core` module `insights` with grouping in SQL where it is natural
   (by block, hour of the session start in local time, weekday, week); tests for each group,
   filters incl. `"none"`, ticks excluded, empty ranges.
3. `review.get` / `review.save` in module `reviews`; summary per plan.md; tests.
4. CLI `salah goal [list|add|edit|end|remove|history]`, `salah insights`, `salah review [show|save]`;
   fixtures named in plan.md "Phase 4"; protocol-v1.md. CHECKPOINT 4.

Additive protocol only (existing fixtures may only gain fields); files under ~800 lines; same
layering as data-model.md principle 4. Checks: `cargo test --offline --workspace` (say which
socket tests your sandbox skips), `cargo clippy --offline --all-targets -- -D warnings`,
`cargo fmt --check`, `git diff --check`.

## Checkpoint protocol

Your sandbox cannot write `.git` or create Unix sockets. Never run `git commit`. At each green
checkpoint (all checks pass except tests that need Unix sockets) append to your progress file a
line `CHECKPOINT <slice>: <one-line commit message>` listing the files, then END your turn. The
orchestrator (Claude session `salah-orch`) runs the socket tests, reviews, commits, and resumes you.
Do not edit files outside your ownership. Never touch ~/.local/share/salah or the live daemon;
do not run ./scripts/install.
