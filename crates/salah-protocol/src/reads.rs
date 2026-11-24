//! Read-only methods. Day reads may write the day's snapshots, which neither
//! bump the revision nor appear in the change log.

use crate::dispatcher::{Call, Context, parse, to_value};
use crate::error::RpcError;
use crate::methods::{
    DayParams, ListParams, ParseParams, PreviewParams, ScopeParams, SessionsListParams,
    StatsParams, StatsSeriesParams, StatsTableParams, SubscribeParams, SubscribeResult,
};
use chrono::{Duration, NaiveDate};
use salah_core::items::ItemView;
use salah_core::stats::measures::Measures;
use salah_core::stats::{Filter, MAX_DAYS, Range};
use salah_core::{
    day, items, metrics, projects, quick, sessions, stats as statistics, status, tags,
};
use salah_domain::error::Error;
use serde_json::Value;

/// Days a report covers when neither a range nor `days` is given.
const DEFAULT_STATS_DAYS: i64 = 30;

pub(crate) fn status(_: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let config = ctx.config.load()?;
    let now = ctx.now();
    let mut report = status::build(&config, now)?;
    report.attach_planner(ctx.store, &config, now);
    to_value(report)
}

pub(crate) fn day(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let params: DayParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let now = ctx.now();
    to_value(day::day_plan(ctx.store, &config, now, params.date)?)
}

pub(crate) fn items(_: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let today = day::current(&ctx.config.load()?, ctx.now())?;
    let rows: Vec<ItemView> = items::list(ctx.store)?
        .into_iter()
        .map(|item| ItemView::new(ctx.store, item, today))
        .collect::<Result<_, _>>()?;
    to_value(rows)
}

pub(crate) fn stats(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let params: StatsParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let today = day::current(&config, ctx.now())?;
    let (range, filter) = scope(params.scope, today, params.measures)?;
    to_value(statistics::stats(ctx.store, &config, today, range, filter)?)
}

pub(crate) fn insights(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let params: ScopeParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let today = day::current(&config, ctx.now())?;
    let (range, filter) = scope(params, today, Measures::default())?;
    to_value(salah_core::insights::get(
        ctx.store, &config, today, range, filter,
    )?)
}

pub(crate) fn stats_table(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let params: StatsTableParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let today = day::current(&config, ctx.now())?;
    let (range, filter) = scope(params.stats.scope, today, params.stats.measures)?;
    to_value(statistics::table::table(
        ctx.store,
        statistics::Calendar {
            config: &config,
            today,
        },
        range,
        filter,
        params.group,
    )?)
}

pub(crate) fn stats_series(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let params: StatsSeriesParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let today = day::current(&config, ctx.now())?;
    let (range, filter) = scope(params.stats.scope, today, params.stats.measures)?;
    to_value(statistics::series::series(
        ctx.store,
        statistics::Calendar {
            config: &config,
            today,
        },
        range,
        filter,
        params.interval,
        params.split,
        &params.measures,
    )?)
}

pub(crate) fn sessions_list(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let params: SessionsListParams = parse(call.params)?;
    let today = day::current(&ctx.config.load()?, ctx.now())?;
    let (range, filter) = scope(params.stats, today, Measures::default())?;
    to_value(sessions::list(ctx.store, range, filter, params.query)?)
}

/// `from` and `to` together, or `days` ending today (default 30), and the filter.
fn scope(
    params: ScopeParams,
    today: NaiveDate,
    measures: Measures,
) -> Result<(Range, Filter), RpcError> {
    let range = match (params.from, params.to, params.days) {
        (Some(from), Some(to), None) => Range { from, to },
        (None, None, days) => {
            let days = days.unwrap_or(DEFAULT_STATS_DAYS);
            if !(1..=MAX_DAYS).contains(&days) {
                return Err(Error::validation(
                    "days",
                    format!("days must be between 1 and {MAX_DAYS}"),
                )
                .into());
            }
            Range {
                from: today
                    .checked_sub_signed(Duration::days(days - 1))
                    .ok_or_else(|| {
                        Error::validation("days", "the range starts before the supported calendar")
                    })?,
                to: today,
            }
        }
        _ => {
            return Err(
                Error::validation("from", "give from and to together, or days alone").into(),
            );
        }
    };
    let filter = Filter {
        tag_id: params.tag_id,
        project_id: params.project_id,
        epic_id: params.epic_id,
        item_id: params.item_id,
        measures,
    };
    Ok((range, filter))
}

pub(crate) fn item_parse(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let params: ParseParams = parse(call.params)?;
    let mut draft = quick::parse(&params.text)?;
    // Only a named day needs the tracking day, so other text parses without config.
    if draft.names_a_day() {
        draft = draft.on(day::current(&ctx.config.load()?, ctx.now())?);
    }
    to_value(draft)
}

/// Previews a draft's recurrence without touching the store.
pub(crate) fn item_preview(call: Call<'_>, _: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: PreviewParams = parse(call.params)?;
    to_value(items::preview(
        p.block,
        p.repeat,
        p.weekdays,
        p.planned_minutes,
        p.date,
    )?)
}

pub(crate) fn backlog(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let params: crate::methods::BacklogListParams = parse(call.params)?;
    let today = day::current(&ctx.config.load()?, ctx.now())?;
    to_value(salah_core::backlog::list(
        ctx.store,
        today,
        params.project_id.as_deref(),
        params.tag_id.as_deref(),
    )?)
}

pub(crate) fn epics(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let params: crate::methods::EpicsListParams = parse(call.params)?;
    let today = day::current(&ctx.config.load()?, ctx.now())?;
    to_value(salah_core::epics::list(
        ctx.store,
        params.project_id.as_deref(),
        params.status,
        today,
    )?)
}

pub(crate) fn projects(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let params: ListParams = parse(call.params)?;
    to_value(projects::list(ctx.store, params.archived)?)
}

pub(crate) fn tags(_: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    to_value(tags::list(ctx.store)?)
}

pub(crate) fn metrics(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let params: ListParams = parse(call.params)?;
    to_value(metrics::list(ctx.store, params.archived)?)
}

pub(crate) fn config(_: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    to_value(ctx.config.load()?)
}

/// Validates the topics and returns the current revision. The socket daemon
/// intercepts `subscribe` to register the connection for pushes.
pub(crate) fn subscribe(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let params: SubscribeParams = parse(call.params)?;
    if let Some(topic) = params
        .topics
        .iter()
        .find(|topic| !matches!(topic.as_str(), "changes" | "clock"))
    {
        return Err(Error::validation(
            "topics",
            format!("unknown topic {topic:?}; supported: changes, clock"),
        )
        .into());
    }
    to_value(SubscribeResult {
        revision: ctx.store.revision()?,
        after_revision: params.after_revision,
    })
}
