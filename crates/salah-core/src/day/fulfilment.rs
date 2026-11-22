//! How much of a day's plan was done.

use salah_domain::tracking::{DayPlanLine, Session};
use serde::Serialize;
use std::collections::BTreeSet;

/// Fulfilment counts only minutes logged against the day's plan lines;
/// sessions of unplanned items or of a bare project are `unplanned_minutes`.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Fulfilment {
    pub planned_items: i64,
    pub done_items: i64,
    pub planned_minutes: i64,
    pub actual_minutes: i64,
    pub unplanned_minutes: i64,
}

impl Fulfilment {
    /// A plan line is done once a session of its item finished that day.
    pub fn of(plan: &[DayPlanLine], sessions: &[Session]) -> Self {
        let planned: BTreeSet<&str> = plan.iter().map(|line| line.item_id.as_str()).collect();
        let (on_plan, off_plan): (Vec<&Session>, Vec<&Session>) = sessions
            .iter()
            .partition(|s| s.item_id.as_deref().is_some_and(|id| planned.contains(id)));
        Self {
            planned_items: plan.len() as i64,
            done_items: plan
                .iter()
                .filter(|line| has_finished(sessions, &line.item_id))
                .count() as i64,
            planned_minutes: plan.iter().filter_map(|line| line.planned_minutes).sum(),
            actual_minutes: total_minutes(on_plan),
            unplanned_minutes: total_minutes(off_plan),
        }
    }

    /// Plan-line minutes over planned minutes, at most 1; `None` without a plan.
    pub fn ratio(&self) -> Option<f64> {
        (self.planned_minutes > 0)
            .then(|| (self.actual_minutes as f64 / self.planned_minutes as f64).min(1.0))
    }
}

/// Logged minutes; running sessions and ticks contribute nothing.
pub fn total_minutes<'a>(sessions: impl IntoIterator<Item = &'a Session>) -> i64 {
    sessions.into_iter().filter_map(|s| s.minutes).sum()
}

pub(super) fn has_finished(sessions: &[Session], item_id: &str) -> bool {
    sessions
        .iter()
        .any(|s| !s.is_running() && s.item_id.as_deref() == Some(item_id))
}
