use crate::{
    dispatcher::{Call, Context, parse, to_value},
    error::RpcError,
    methods::Changed,
    writes::mutate,
};
use salah_core::{day, reviews};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Get {
    week: String,
}
#[derive(Deserialize)]
struct Notes {
    week: String,
    query: String,
    limit: Option<i64>,
    after: Option<String>,
}
#[derive(Deserialize)]
struct Save {
    week: String,
    note: String,
    op_id: String,
}

pub(crate) fn get(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: Get = parse(call.params)?;
    let week = reviews::week(&p.week)?;
    let today = day::current(&ctx.config.load()?, ctx.now())?;
    to_value(reviews::get(ctx.store, week, today)?)
}
pub(crate) fn notes(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: Notes = parse(call.params)?;
    to_value(reviews::notes(
        ctx.store,
        reviews::week(&p.week)?,
        &p.query,
        p.limit,
        p.after.as_deref(),
    )?)
}

pub(crate) fn save(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: Save = parse(call.params)?;
    let (now, ids) = (ctx.now(), ctx.ids);
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        let week = reviews::week(&p.week)?;
        let mut changed = Changed::default();
        if let Some(row) = reviews::save(tx, week, &p.note, now, ids)? {
            if row.deleted_at.is_some() {
                changed.deleted.push(row.id);
            } else {
                changed.reviews.push(row);
            }
        }
        Ok(changed)
    })
}
