//! Read-only methods. Day reads may write the day's snapshots, which neither
//! bump the revision nor appear in the change log.

use crate::dispatcher::{Call, Context, parse, to_value};
use crate::error::RpcError;
use crate::methods::{
    DayParams, ListParams, ParseParams, PreviewParams, StatsParams, SubscribeParams,
    SubscribeResult,
};
use chrono::Duration;
use salah_core::items::ItemView;
use salah_core::stats::{Filter, MAX_DAYS, Range};
use salah_core::{day, items, metrics, projects, quick, stats as statistics, status, tags};
use salah_domain::error::Error;
use serde_json::Value;

/// Days `stats.get` covers when neither a range nor `days` is given.
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
            let to = today;
            Range {
                from: to
                    .checked_sub_signed(Duration::days(days - 1))
                    .ok_or_else(|| {
                        Error::validation("days", "the range starts before the supported calendar")
                    })?,
                to,
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
        item_id: params.item_id,
    };
    to_value(statistics::stats(ctx.store, &config, today, range, filter)?)
}

pub(crate) fn item_parse(call: Call<'_>, _: &mut Context<'_>) -> Result<Value, RpcError> {
    let params: ParseParams = parse(call.params)?;
    to_value(quick::parse(&params.text)?)
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
