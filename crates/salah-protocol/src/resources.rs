//! `resources.list` and `resource.add/edit/remove`: notes, links and
//! reminders on one item or epic.

use crate::dispatcher::{Call, Context, parse, to_value};
use crate::error::RpcError;
use crate::methods::{Changed, RemoveParams};
use crate::writes::mutate;
use salah_core::resources::{self, NewResource, Owner, ResourceChanges};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
pub struct ResourceAddParams {
    pub op_id: String,
    #[serde(flatten)]
    pub resource: NewResource,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ResourceEditParams {
    pub op_id: String,
    pub id: String,
    #[serde(default)]
    pub expected_version: Option<i64>,
    #[serde(flatten)]
    pub changes: ResourceChanges,
}

pub(crate) fn list(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let owner: Owner = parse(call.params)?;
    to_value(resources::list(ctx.store, &owner)?)
}

pub(crate) fn add(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: ResourceAddParams = parse(call.params)?;
    let (now, ids) = (ctx.now(), ctx.ids);
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        Ok(Changed {
            resources: vec![resources::add(tx, now, ids, p.resource.clone())?],
            ..Changed::default()
        })
    })
}

pub(crate) fn edit(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: ResourceEditParams = parse(call.params)?;
    let now = ctx.now();
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        Ok(Changed {
            resources: vec![resources::edit(
                tx,
                now,
                &p.id,
                p.changes.clone(),
                p.expected_version,
            )?],
            ..Changed::default()
        })
    })
}

pub(crate) fn remove(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: RemoveParams = parse(call.params)?;
    let now = ctx.now();
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        let removed = resources::remove(tx, now, &p.id, p.expected_version)?;
        Ok(Changed {
            deleted: vec![removed.id],
            ..Changed::default()
        })
    })
}
