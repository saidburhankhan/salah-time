# CLI contract v1: frozen fixtures

This is the golden contract the desktop app and the bar widget consume today:
the unwrapped JSON printed by the `salah` CLI. `tests/contract.rs` runs the real
binary against an isolated `SALAH_CONFIG`/`SALAH_DATA`, seeds state through the
CLI, normalises clock- and id-dependent values, and compares the result with
`tests/fixtures/contract/<name>.json`.

Regenerate fixtures deliberately, from the current binary:

```sh
UPDATE_FIXTURES=1 cargo test --offline --test contract
cargo test --offline --test contract
```

## Commands and fixtures

| Command | Fixture(s) |
|---|---|
| `salah status --json` | `status` |
| `salah item list --json` | `item_list` |
| `salah day --json --date D` | `day` |
| `salah stats --json --from D --to D` | `stats_range` |
| `salah stats --json --days N` | `stats_days` |
| `salah item parse --json -- TEXT` | `item_parse`, `item_parse_weekdays` |
| `salah item add --json -- TITLE` | `item_add_plain`, `item_add_date` |
| `salah item add --json --quick -- TEXT` | `item_add_quick` |
| `salah item add --json --block B -- TITLE` | `item_add_block` |
| `salah item add --json --repeat daily -- TITLE` | `item_add_daily` |
| `salah item add --json --repeat weekdays -- TITLE` | `item_add_weekdays` |
| `salah item add --json --repeat weekly --days … -- TITLE` | `item_add_weekly_days` |
| `salah item add --json --measure count --target N -- TITLE` | `item_add_count_target` |
| `salah item edit --json ID --title=T` | `item_edit_title` |
| `salah item edit --json ID --block B` | `item_edit_block` |
| `salah item edit --json ID --repeat weekly --days …` | `item_edit_repeat` |
| `salah item edit --json ID --target N` | `item_edit_target` |
| `salah item edit --json ID --no-target` | `item_edit_no_target` |
| `salah item move --json ID --block B --index N` | `item_move` |
| `salah item done --json ID --date D` | `item_done` |
| `salah item undo --json ID --date D` | `item_undo` |
| `salah item log --json ID VALUE --date D` | `item_log_positive`, `item_log_negative`, `item_log_count` |
| `salah item log --json --minutes ID VALUE --date D` | `item_log_minutes_positive`, `item_log_minutes_negative` |
| `salah item start --json ID` | `item_start` |
| `salah item stop --json ID` | `item_stop_id` |
| `salah item stop --json` | `item_stop` |
| `salah item remove --json ID` | `item_remove` |
| `salah prayer mark --json PRAYER --date D` | `prayer_mark` |
| `salah prayer unmark --json PRAYER --date D` | `prayer_unmark` |
| `salah config show` | `config_show` |
| `salah export` | `export` |
| `salah import FILE --json` | `import`, `import_again` (idempotent re-import) |
| `salah item done UNKNOWN --json` | `error_unknown_id` |
| `salah item add --repeat weekly …` (no `--days`) | `error_weekly_without_days` |
| `salah item add --block sleep …` | `error_sleep_block` |
| `salah item log … 0` | `error_zero_log` |
| `salah day --date NOT-A-DATE --json` | `error_bad_date` |

Every error fixture stores the exact stderr string, including the `salah: `
prefix. A failed command must exit non-zero and must not change planner state.

## Normalisation

Fixture values that the real clock or random ids make volatile are replaced
before comparison. Structure, field names, JSON types, enum values, ordering,
and null-ness are otherwise compared exactly.

- UUIDs become `<id:N>`, numbered by first appearance in each fixture.
- RFC 3339 instants become `<instant>`.
- Only dates equal to today, yesterday, or tomorrow become `<today>`,
  `<today-1>`, or `<today+1>`. Explicit far dates stay literal.
- `stats --days` dates become `<to>`, `<to-N>`, … relative to the response's
  `to`, so a rolling window is stable.
- Clock strings (`HH:MM`) become `<HH:MM>`.
- `remaining_seconds` becomes `0` (a number, keeping the field type);
  countdown strings (`remaining`, `remaining_short`, `label`) become
  `<duration>`.
- Time-of-day-dependent `status` fields (`next`, `current_block`, `passed`,
  `is_next`, `current`, `sleeping`, `planner.block`, `planning_block.start`,
  `label`) use structural placeholders (`<boundary>`, `<name>`, `<kind>`).
  `item list` normalises its current `block` and `planning_block.start`.
  Timer entries normalise the current `block` day and start.

These last two groups cannot be frozen literally without injecting a clock:
the current CLI calls `Utc::now()` directly. The fixtures freeze their shape,
types, and ordering, not which boundary happens to be next when the test runs.

## Evolution rules

- **v1 is additive.** New optional fields may be added at any time; existing
  fields keep their name, type, meaning, null-ness, and ordering.
- **Enum values are part of the contract.** Adding a value to `prayer`,
  `block`, `repeat`, `measure`, `state`, or an error class is breaking for
  strict consumers and needs a contract review before it ships in v1.
- **Removing, renaming, retyping, or changing the meaning** of any field or
  command is a breaking change and requires a v2 shape alongside v1.
- **Error text is frozen only as a fixture.** The `salah: ` prefix and non-zero
  exit are v1; internal wording may improve without a v2, but the fixture must
  be updated deliberately in the same change.
- Fixture changes must be intentional and reviewed as contract changes. Do not
  regenerate them to make an accidental behaviour change pass.
