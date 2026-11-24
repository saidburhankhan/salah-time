//! Day and Monday-first week targets. Session progress is computed on read.
use crate::{
    Store, WriteTx,
    guard::{ensure_version, not_found},
    stats,
};
use chrono::{DateTime, NaiveDate, Utc};
use salah_domain::{
    error::Error,
    goal::{Direction, Goal, GoalView, HistoryRow, Period, Progress, Scope},
    runtime::IdGenerator,
    tracking::Rollup,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct NewGoal {
    pub scope: Scope,
    pub period: Period,
    pub metric_id: Option<String>,
    pub direction: Direction,
    pub target: f64,
}

/// Parse an external calendar date without chrono's abbreviated month/day or extended years.
pub fn parse_day(field: &'static str, s: &str) -> Result<NaiveDate, Error> {
    s.parse::<NaiveDate>()
        .ok()
        .filter(|d| s.len() == 10 && d.to_string() == s)
        .ok_or_else(|| Error::validation(field, "use a valid YYYY-MM-DD date"))
}

/// Compare the weighted numerator to target × weight before rounding the
/// quotient. Keep the low-order terms of both the products and their sum.
/// All metric samples and weights are finite and nonnegative.
fn weighted_progress(samples: &[(f64, f64)], target: f64) -> (f64, std::cmp::Ordering) {
    let weight: f64 = samples.iter().map(|(_, w)| w).sum();
    let largest = samples.iter().map(|(v, _)| *v).fold(0., f64::max);
    // Scale the weights by an exact power of two only if the numerator
    // might overflow. The mean of even the largest finite counts is finite.
    let divisor = if largest >= (f64::MAX / 2.) / weight {
        2.0_f64.powi(weight.log2().ceil() as i32 + 1)
    } else {
        1.
    };
    let weight = weight / divisor;
    let mut sum = 0.;
    let mut correction = 0.;
    for &(v, w) in samples {
        let w = w / divisor;
        let product = v * w;
        let next = sum + product;
        // Neumaier summation also handles a term larger than the partial sum.
        correction += if sum >= product {
            (sum - next) + product
        } else {
            (product - next) + sum
        };
        correction += v.mul_add(w, -product);
        sum = next;
    }
    let threshold = target * weight;
    let ordering = if threshold.is_infinite() {
        std::cmp::Ordering::Less
    } else {
        let residual = (sum - threshold) + (correction - target.mul_add(weight, -threshold));
        residual
            .partial_cmp(&0.)
            .expect("finite weighted difference")
    };
    let value = if ordering.is_eq() {
        target
    } else {
        ((sum + correction) / weight).min(largest)
    };
    (value, ordering)
}

fn progress(
    goal: &Goal,
    from: NaiveDate,
    to: NaiveDate,
    rollup: Rollup,
    samples: &[(f64, f64)],
) -> Progress {
    let (value, comparison) = if rollup == Rollup::Avg && !samples.is_empty() {
        let (value, comparison) = weighted_progress(samples, goal.target);
        (Some(value), Some(comparison))
    } else {
        let value = stats::aggregate(rollup, samples.iter().copied())
            .or_else(|| goal.metric_id.is_none().then_some(0.));
        (value, value.and_then(|v| v.partial_cmp(&goal.target)))
    };
    let overflow = value.is_some_and(|v| !v.is_finite());
    let met = !overflow
        && comparison.is_some_and(|ordering| match goal.direction {
            Direction::AtLeast => ordering.is_ge(),
            Direction::AtMost => ordering.is_le(),
        });
    Progress {
        from,
        to,
        value: value.filter(|v| v.is_finite()),
        met,
        error: overflow.then_some("sum_overflow"),
    }
}

/// One read per version for the full range, including empty periods.
fn periods(
    store: &Store,
    g: &Goal,
    first: NaiveDate,
    last: NaiveDate,
) -> Result<Vec<HistoryRow>, Error> {
    let mut from = g.period.bounds(first)?.0.max(g.starts_on);
    let through = g
        .period
        .bounds(last)?
        .1
        .min(g.ends_on.unwrap_or(NaiveDate::MAX));
    if from > through {
        return Ok(vec![]);
    }
    let rollup = match &g.metric_id {
        None => Rollup::Sum,
        Some(id) => {
            store
                .metric(id)?
                .ok_or_else(|| not_found("metric", id))?
                .rollup
        }
    };
    let mut groups = BTreeMap::<NaiveDate, Vec<(f64, f64)>>::new();
    for (day, value, weight) in store.goal_samples(g, from, through)? {
        groups
            .entry(g.period.bounds(day)?.0)
            .or_default()
            .push((value, weight));
    }
    if rollup == Rollup::Avg {
        // Compare weighted numerators in the same sample order across query plans.
        for samples in groups.values_mut() {
            samples.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        }
    }
    let mut rows = Vec::new();
    loop {
        let to = g.period.bounds(from)?.1;
        rows.push(HistoryRow {
            progress: progress(
                g,
                from,
                to,
                rollup,
                groups.get(&from).map(Vec::as_slice).unwrap_or_default(),
            ),
            target: g.target,
        });
        if to >= through {
            break;
        }
        from = to
            .succ_opt()
            .ok_or_else(|| Error::validation("to", "date out of range"))?;
    }
    Ok(rows)
}

/// Met daily and weekly periods in the selected review week.
pub(crate) fn met_between(
    store: &Store,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<GoalView>, Error> {
    let mut met = Vec::new();
    if from > to {
        return Ok(met);
    }
    for g in store.goals_between(from, to)? {
        let label = store.goal_scope_label(&g.scope)?;
        for row in periods(store, &g, from, to)? {
            if row.progress.met {
                met.push(GoalView {
                    goal: g.clone(),
                    scope_label: label.clone(),
                    progress: row.progress,
                });
            }
        }
    }
    Ok(met)
}

pub fn view(store: &Store, g: Goal, at: NaiveDate) -> Result<GoalView, Error> {
    let at = at.min(g.ends_on.unwrap_or(at)).max(g.starts_on);
    let p = periods(store, &g, at, at)?
        .pop()
        .ok_or_else(|| Error::Storage("goal has no valid period".into()))?;
    Ok(GoalView {
        scope_label: store.goal_scope_label(&g.scope)?,
        goal: g,
        progress: p.progress,
    })
}

pub fn list(
    store: &Store,
    config: &crate::config::Config,
    at: NaiveDate,
    include_ended: bool,
) -> Result<Vec<GoalView>, Error> {
    store.read_snapshot(|store| {
        let mut goals = store.goals_at(at, include_ended)?;
        if !include_ended && goals.iter().any(|g| g.ends_on.is_some()) {
            // An ended, non-deleted version's last update is its closing instant.
            // A close exactly at Sleep belongs to the next tracking day.
            let sleep = crate::day::historical_clock(store, config, at)?.sleep_at;
            goals.retain(|g| g.ends_on.is_none() || g.updated_at >= sleep);
        }
        goals.into_iter().map(|g| view(store, g, at)).collect()
    })
}

/// Each period of the goal's whole edit chain (any member's id) within
/// `from..=to`, up to today's period, with the target that period had.
pub fn history(
    store: &Store,
    id: &str,
    from: NaiveDate,
    to: NaiveDate,
    today: NaiveDate,
) -> Result<Vec<HistoryRow>, Error> {
    if from > to {
        return Err(Error::validation("to", "range ends before its start"));
    }
    if (to - from).num_days() >= stats::MAX_DAYS {
        return Err(Error::validation(
            "to",
            format!("a range covers at most {} days", stats::MAX_DAYS),
        ));
    }
    store.read_snapshot(|store| {
        let versions = store.goal_history_versions(id)?;
        if versions.is_empty() {
            return Err(not_found("goal", id));
        }
        let mut rows = Vec::new();
        for goal in versions {
            let first = from.max(goal.starts_on);
            let last = to.min(today).min(goal.ends_on.unwrap_or(to));
            if first <= last {
                rows.extend(periods(store, &goal, first, last)?);
            }
        }
        Ok(rows)
    })
}

pub fn add(
    tx: &WriteTx<'_>,
    new: NewGoal,
    now: DateTime<Utc>,
    today: NaiveDate,
    ids: &dyn IdGenerator,
) -> Result<GoalView, Error> {
    let g = Goal {
        id: ids.new_id(),
        scope: new.scope,
        period: new.period,
        metric_id: new.metric_id,
        direction: new.direction,
        target: new.target,
        starts_on: new.period.bounds(today)?.0,
        ends_on: None,
        replaces_id: None,
        position: tx.next_goal_position()?,
        created_at: now,
        updated_at: now,
        deleted_at: None,
        version: 1,
    };
    tx.insert_goal(&g)?;
    view(tx, g, today)
}

fn active(store: &Store, id: &str, today: NaiveDate, expected: Option<i64>) -> Result<Goal, Error> {
    let g = store
        .goal(id)?
        .filter(|g| g.deleted_at.is_none())
        .ok_or_else(|| not_found("goal", id))?;
    ensure_version("goal", id, g.version, expected)?;
    if g.ends_on.is_some() || today < g.starts_on {
        return Err(Error::validation("id", "goal is outside its active period"));
    }
    Ok(g)
}

pub struct EditResult {
    pub previous: Goal,
    pub successor: GoalView,
}

pub fn edit(
    tx: &WriteTx<'_>,
    id: &str,
    target: f64,
    expected: Option<i64>,
    now: DateTime<Utc>,
    today: NaiveDate,
    ids: &dyn IdGenerator,
) -> Result<EditResult, Error> {
    let mut old = active(tx, id, today, expected)?;
    let start = old.period.bounds(today)?.0;
    let successor = Goal {
        id: ids.new_id(),
        target,
        starts_on: start,
        replaces_id: Some(old.id.clone()),
        created_at: now,
        updated_at: now,
        version: 1,
        ..old.clone()
    };
    if start == old.starts_on {
        old.deleted_at = Some(now);
    } else {
        old.ends_on = start.pred_opt();
    }
    old.updated_at = now;
    old.version = tx.close_goal(&old)?;
    tx.insert_goal(&successor)?;
    Ok(EditResult {
        previous: old,
        successor: view(tx, successor, today)?,
    })
}

fn close(
    tx: &WriteTx<'_>,
    mut g: Goal,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<Goal, Error> {
    let (start, end) = g.period.bounds(today)?;
    g.ends_on = Some(if start <= g.starts_on {
        g.period.bounds(g.starts_on)?.1
    } else {
        start.pred_opt().unwrap_or(end)
    });
    g.updated_at = now;
    g.version = tx.close_goal(&g)?;
    Ok(g)
}

pub fn end(
    tx: &WriteTx<'_>,
    id: &str,
    expected: Option<i64>,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<GoalView, Error> {
    view(
        tx,
        close(tx, active(tx, id, today, expected)?, today, now)?,
        today,
    )
}

/// Ends the active goals of a removed project, epic or task at their last
/// full period (or their first period's end); they are never deleted.
pub(crate) fn end_scope(
    tx: &WriteTx<'_>,
    scope: Scope,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<(), Error> {
    for g in tx.goals_for_scope(&scope)? {
        close(tx, g, today, now)?;
    }
    Ok(())
}

/// Runs `remove` (which ends the goals of `scope`) and returns the goals it
/// ended, so a removal's result can list them.
pub fn ending_scope<T>(
    store: &Store,
    scope: &Scope,
    today: NaiveDate,
    remove: impl FnOnce() -> Result<T, Error>,
) -> Result<(T, Vec<GoalView>), Error> {
    let active = store.goals_for_scope(scope)?;
    let removed = remove()?;
    let mut ended = Vec::new();
    for g in active {
        if let Some(g) = store.goal(&g.id)? {
            ended.push(view(store, g, today)?);
        }
    }
    Ok((removed, ended))
}

pub fn remove(
    tx: &WriteTx<'_>,
    id: &str,
    expected: Option<i64>,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<(), Error> {
    let mut g = tx
        .goal(id)?
        .filter(|g| g.deleted_at.is_none())
        .ok_or_else(|| not_found("goal", id))?;
    ensure_version("goal", id, g.version, expected)?;
    if g.period.bounds(today)?.0 != g.starts_on {
        return Err(Error::validation(
            "id",
            "only a goal in its first period can be removed",
        ));
    }
    g.updated_at = now;
    g.deleted_at = Some(now);
    tx.close_goal(&g)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::weighted_progress;
    use std::cmp::Ordering;

    #[test]
    fn weighted_comparison_preserves_large_means_and_nearby_unequal_targets() {
        assert_eq!(
            weighted_progress(&[(1e308, 10.), (0., 10.)], 5e307),
            (5e307, Ordering::Equal)
        );
        assert_eq!(
            weighted_progress(&[(f64::MAX, 11.)], f64::MAX),
            (f64::MAX, Ordering::Equal)
        );
        assert_eq!(
            weighted_progress(&[(2., 1.), (2_f64.next_up(), 1.)], 2.).1,
            Ordering::Greater
        );
        assert_eq!(
            weighted_progress(&[(2_f64.next_down(), 1.), (2., 1.)], 2.).1,
            Ordering::Less
        );
        assert_eq!(
            weighted_progress(&[(1., 1.), (2., 11.), (3., 1.)], f64::MAX).1,
            Ordering::Less
        );
    }
}
