//! Goal queries and replayable mutations.
use crate::{
    dispatcher::{Call, Context, parse, to_value},
    error::RpcError,
    methods::{Changed, RemoveParams},
    writes::mutate,
};
use salah_core::{
    day,
    goals::{self, NewGoal, parse_day},
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct List {
    at: Option<String>,
    #[serde(default)]
    include_ended: bool,
}
#[derive(Deserialize)]
struct History {
    id: String,
    from: String,
    to: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Add {
    op_id: String,
    project_id: Option<String>,
    epic_id: Option<String>,
    item_id: Option<String>,
    metric_id: Option<String>,
    period: salah_domain::goal::Period,
    #[serde(default)]
    direction: salah_domain::goal::Direction,
    target: f64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    op_id: String,
    id: String,
    target: f64,
    expected_version: Option<i64>,
}

pub(crate) fn list(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: List = parse(call.params)?;
    let config = ctx.config.load()?;
    let at = match p.at {
        Some(s) => parse_day("at", &s)?,
        None => day::current(&config, ctx.now())?,
    };
    to_value(goals::list(ctx.store, &config, at, p.include_ended)?)
}
pub(crate) fn history(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: History = parse(call.params)?;
    let today = day::current(&ctx.config.load()?, ctx.now())?;
    to_value(goals::history(
        ctx.store,
        &p.id,
        parse_day("from", &p.from)?,
        parse_day("to", &p.to)?,
        today,
    )?)
}
pub(crate) fn add(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: Add = parse(call.params)?;
    let (now, ids) = (ctx.now(), ctx.ids);
    let today = day::current(&ctx.config.load()?, now)?;
    let new = NewGoal {
        scope: salah_domain::goal::Scope {
            project_id: p.project_id,
            epic_id: p.epic_id,
            item_id: p.item_id,
        },
        metric_id: p.metric_id,
        period: p.period,
        direction: p.direction,
        target: p.target,
    };
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        Ok(Changed {
            goals: vec![goals::add(tx, new.clone(), now, today, ids)?],
            ..Changed::default()
        })
    })
}
pub(crate) fn edit(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: Edit = parse(call.params)?;
    let (now, ids) = (ctx.now(), ctx.ids);
    let today = day::current(&ctx.config.load()?, now)?;
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        let change = goals::edit(tx, &p.id, p.target, p.expected_version, now, today, ids)?;
        let mut result = Changed {
            goals: vec![change.successor],
            ..Changed::default()
        };
        if change.previous.deleted_at.is_some() {
            result.deleted.push(change.previous.id);
        } else {
            result.goals.push(goals::view(tx, change.previous, today)?);
        }
        Ok(result)
    })
}
pub(crate) fn end(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: RemoveParams = parse(call.params)?;
    let now = ctx.now();
    let today = day::current(&ctx.config.load()?, now)?;
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        Ok(Changed {
            goals: vec![goals::end(tx, &p.id, p.expected_version, today, now)?],
            ..Changed::default()
        })
    })
}
pub(crate) fn remove(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: RemoveParams = parse(call.params)?;
    let now = ctx.now();
    let today = day::current(&ctx.config.load()?, now)?;
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        goals::remove(tx, &p.id, p.expected_version, today, now)?;
        Ok(Changed {
            deleted: vec![p.id.clone()],
            ..Changed::default()
        })
    })
}
