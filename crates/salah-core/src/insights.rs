//! Session insights over the shared report scope. The planning day's clock
//! supplies both the local start hour and the boundaries for block allocation.
use crate::{
    Store,
    config::Config,
    day,
    sessions::notes::{NotePreview, truncate_to_bytes},
    stats::{Filter, Range, aggregate, report_scope},
};
use chrono::{Duration, NaiveDate, Timelike};
use salah_domain::{
    day::BoundaryId,
    error::Error,
    goal::Period,
    reports::{distraction_total, distractions_per_hour},
    tracking::{Rollup, Session},
};
use salah_store::reports::{CalendarGroup, GroupMeasures, ReportSession};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize)]
pub struct Measures {
    minutes: i64,
    sessions: i64,
    focus_avg: Option<f64>,
    distractions_per_hour: Option<f64>,
}

impl From<GroupMeasures> for Measures {
    fn from(m: GroupMeasures) -> Self {
        Self {
            minutes: m.minutes,
            sessions: m.sessions,
            focus_avg: m.focus_avg,
            distractions_per_hour: m.distractions_per_hour,
        }
    }
}

#[derive(Default)]
struct Samples {
    minutes: i64,
    sessions: i64,
    focus: Vec<(f64, f64)>,
    distractions: Vec<f64>,
}

impl Samples {
    fn add(&mut self, s: &Session, minutes: i64) {
        self.sessions += 1;
        self.minutes += minutes;
        let weight = minutes.max(1) as f64;
        if let Some(focus) = s.value("focus") {
            self.focus.push((focus, weight));
        }
        if let Some(count) = s.value("distractions") {
            self.distractions.push(count * (weight / s.weight()));
        }
    }
    fn finish(self) -> Measures {
        Measures {
            minutes: self.minutes,
            sessions: self.sessions,
            focus_avg: aggregate(Rollup::Avg, self.focus.into_iter()),
            distractions_per_hour: distractions_per_hour(
                distraction_total(self.distractions.into_iter()),
                self.minutes,
            ),
        }
    }
}

#[derive(Serialize)]
struct BlockRow {
    block: BoundaryId,
    #[serde(flatten)]
    measures: Measures,
}
#[derive(Serialize)]
struct HourRow {
    hour: u32,
    #[serde(flatten)]
    measures: Measures,
}
#[derive(Serialize)]
struct WeekdayRow {
    weekday: u32,
    #[serde(flatten)]
    measures: Measures,
}
#[derive(Serialize)]
struct WeekRow {
    week: NaiveDate,
    per_hour: Option<f64>,
}
#[derive(Serialize)]
struct PlanRow {
    item_id: Option<String>,
    name: String,
    planned_block: Option<BoundaryId>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    planned_blocks: BTreeSet<BoundaryId>,
    minutes: i64,
    scored_minutes: i64,
    on_plan_minutes: i64,
}
#[derive(Serialize)]
struct MatrixCell {
    planned_block: Option<BoundaryId>,
    actual_block: BoundaryId,
    minutes: i64,
}
#[derive(Serialize)]
pub struct Insights {
    by_block: Vec<BlockRow>,
    by_hour: Vec<HourRow>,
    by_weekday: Vec<WeekdayRow>,
    distractions_by_week: Vec<WeekRow>,
    best: Vec<NotePreview>,
    hardest: Vec<NotePreview>,
    on_plan: Vec<PlanRow>,
    block_matrix: Vec<MatrixCell>,
}

fn ranked(store: &Store, rows: &[ReportSession], best: bool) -> Result<Vec<NotePreview>, Error> {
    let mut scored: Vec<_> = rows
        .iter()
        .filter_map(|row| Some((row, row.session.value("focus")?)))
        .collect();
    scored.sort_by(|(a, x), (b, y)| {
        let focus = if best { y.total_cmp(x) } else { x.total_cmp(y) };
        focus
            .then(b.session.minutes.cmp(&a.session.minutes))
            .then(b.sort_at().cmp(&a.sort_at()))
            .then(a.session.id.cmp(&b.session.id))
    });
    scored.truncate(5);
    let mut previews = scored
        .into_iter()
        .map(|(row, _)| NotePreview::new(store, row))
        .collect::<Result<Vec<_>, _>>()?;
    const RANKED_BYTES: usize = 192 * 1024;
    truncate_to_bytes(&mut previews, RANKED_BYTES)?;
    Ok(previews)
}

pub fn get(
    store: &Store,
    config: &Config,
    today: NaiveDate,
    range: Range,
    filter: Filter,
) -> Result<Insights, Error> {
    store.read_snapshot(|store| {
        let mut scope = report_scope(store, &range, &filter)?;
        scope.measures.minutes_min = Some(scope.measures.minutes_min.unwrap_or(0));
        let rows = store.report_sessions(&scope, None)?;
        let (unfiltered, matched) = if filter.measures != Default::default() {
            let matched = rows
                .iter()
                .map(|row| row.session.id.as_str())
                .collect::<BTreeSet<_>>();
            let mut plan_scope = scope.clone();
            plan_scope.measures = salah_store::reports::SessionFilter {
                minutes_min: Some(0),
                ..Default::default()
            };
            (
                Some(store.report_sessions(&plan_scope, None)?),
                Some(matched),
            )
        } else {
            (None, None)
        };
        let mut weekdays = store.calendar_groups(&scope, CalendarGroup::Weekday)?;
        let mut weeks = store.calendar_groups(&scope, CalendarGroup::Week)?;
        let mut distractions_by_week = Vec::new();
        let mut week = Period::Week.bounds(range.from)?.0;
        while week <= range.to {
            distractions_by_week.push(WeekRow {
                week,
                per_hour: weeks
                    .remove(&week.to_string())
                    .and_then(|m| m.distractions_per_hour),
            });
            week += Duration::weeks(1);
        }

        let mut blocks = BTreeMap::<BoundaryId, Samples>::new();
        let mut hours = BTreeMap::<u32, Samples>::new();
        let mut matrix = BTreeMap::new();
        let mut plans = BTreeMap::new();
        let mut bare_minutes = 0;
        // report_sessions is ordered by planning day. Load that day's frozen
        // plan once, including the task's block before any subsequent moves.
        for group in unfiltered
            .as_deref()
            .unwrap_or(&rows)
            .chunk_by(|a, b| a.session.block.day == b.session.block.day)
        {
            let snap = day::snapshot(store, config, group[0].session.block.day, today)?;
            let paused_items = if snap.paused_projects.is_empty() {
                BTreeSet::new()
            } else {
                store.paused_items_on(group[0].session.block.day)?
            };
            let planned: BTreeMap<_, _> = snap
                .plan
                .iter()
                .map(|line| (line.item_id.as_str(), line.block))
                .collect();
            for row in group {
                let s = &row.session;
                let planned_block = s.item_id.as_deref().and_then(|id| planned.get(id)).copied();
                let minutes = s.minutes.expect("recorded minutes in report scope");
                if s.item_id.is_none() {
                    bare_minutes += minutes;
                }
                let matches = matched
                    .as_ref()
                    .is_none_or(|ids| ids.contains(s.id.as_str()));
                if matches && let Some(start) = s.started_at {
                    let hour =
                        (start + Duration::seconds(i64::from(snap.clock.offset_seconds))).hour();
                    hours.entry(hour).or_default().add(s, minutes);
                }
                let allocations = day::block_minutes(s, &snap.blocks);
                for &(actual, part) in &allocations {
                    if matches {
                        blocks.entry(actual).or_default().add(s, part);
                    }
                    *matrix.entry((planned_block, actual)).or_insert(0) += part;
                }
                if let Some(item) = &row.item {
                    let plan = plans.entry(item.id.clone()).or_insert_with(|| PlanRow {
                        item_id: Some(item.id.clone()),
                        name: item.title.clone(),
                        planned_block: None,
                        planned_blocks: BTreeSet::new(),
                        minutes: 0,
                        scored_minutes: 0,
                        on_plan_minutes: 0,
                    });
                    plan.minutes += minutes;
                    if !paused_items.contains(&item.id) {
                        plan.scored_minutes += minutes;
                    }
                    if let Some(block) = planned_block {
                        plan.planned_blocks.insert(block);
                        plan.on_plan_minutes += allocations
                            .iter()
                            .filter(|(actual, _)| *actual == block)
                            .map(|(_, part)| part)
                            .sum::<i64>();
                    }
                }
            }
        }
        let mut on_plan: Vec<_> = plans.into_values().collect();
        for row in &mut on_plan {
            if row.planned_blocks.len() == 1 {
                row.planned_block = row.planned_blocks.first().copied();
            }
        }
        on_plan.sort_by(|a, b| {
            (&a.name, &a.item_id, a.planned_block).cmp(&(&b.name, &b.item_id, b.planned_block))
        });
        if bare_minutes > 0 {
            on_plan.push(PlanRow {
                item_id: None,
                name: "Unplanned".into(),
                planned_block: None,
                planned_blocks: BTreeSet::new(),
                minutes: bare_minutes,
                scored_minutes: 0,
                on_plan_minutes: 0,
            });
        }
        Ok(Insights {
            by_block: BoundaryId::AWAKE
                .into_iter()
                .chain([BoundaryId::Sleep])
                .map(|block| BlockRow {
                    block,
                    measures: blocks.remove(&block).unwrap_or_default().finish(),
                })
                .collect(),
            by_hour: (0..24)
                .map(|hour| HourRow {
                    hour,
                    measures: hours.remove(&hour).unwrap_or_default().finish(),
                })
                .collect(),
            by_weekday: (1..=7)
                .map(|weekday| WeekdayRow {
                    weekday,
                    measures: weekdays
                        .remove(&weekday.to_string())
                        .unwrap_or_default()
                        .into(),
                })
                .collect(),
            distractions_by_week,
            best: ranked(store, &rows, true)?,
            hardest: ranked(store, &rows, false)?,
            on_plan,
            block_matrix: matrix
                .into_iter()
                .map(|((planned_block, actual_block), minutes)| MatrixCell {
                    planned_block,
                    actual_block,
                    minutes,
                })
                .collect(),
        })
    })
}
