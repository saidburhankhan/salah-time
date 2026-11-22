//! Mutating methods. Each runs in one write transaction that also records its
//! complete result under the client's `op_id`, so a retry returns the same
//! bytes and writes nothing.

use crate::dispatcher::{Context, parse, to_value};
use crate::error::RpcError;
use crate::methods::*;
use chrono::NaiveDate;
use salah_core::items::{ItemView, MovePosition, NewItem};
use salah_core::projects::ProjectView;
use salah_core::sessions::SessionView;
use salah_core::{Store, WriteTx, day, items, marks, metrics, projects, quick, sessions, tags};
use salah_domain::error::Error;
use salah_domain::tracking::{Item, Project, Session};
use serde_json::Value;

/// Runs `work` once per `op_id`: a replay returns the stored result before
/// any validation or write.
fn mutate(
    store: &mut Store,
    op_id: &str,
    mut work: impl FnMut(&WriteTx<'_>) -> Result<Changed, Error>,
) -> Result<Value, RpcError> {
    let result = store.write(|tx| {
        tx.set_op_id(Some(op_id));
        if let Some(applied) = tx.applied_op(op_id)? {
            return serde_json::from_str(&applied.result_json)
                .map_err(|error| Error::Storage(format!("invalid applied op result: {error}")));
        }
        let changed = work(tx)?;
        let result = serde_json::to_value(MutationResult {
            revision: tx.revision()?,
            op_id: op_id.to_owned(),
            changed,
        })
        .map_err(|error| Error::Internal(format!("serialize result: {error}")))?;
        tx.record_applied_op(op_id, &result.to_string())?;
        Ok(result)
    })?;
    Ok(result)
}

fn item_views(today: NaiveDate, rows: Vec<Item>) -> Changed {
    Changed {
        items: rows
            .into_iter()
            .map(|item| ItemView::new(item, today))
            .collect(),
        ..Changed::default()
    }
}

fn project_view(store: &Store, project: Project) -> Result<Changed, Error> {
    Ok(Changed {
        projects: vec![projects::view(store, project)?],
        ..Changed::default()
    })
}

fn session_view(session: &Session) -> Changed {
    Changed {
        sessions: vec![SessionView::from(session)],
        ..Changed::default()
    }
}

fn removed(id: String) -> Changed {
    Changed {
        deleted: vec![id],
        ..Changed::default()
    }
}

pub(crate) fn item_add(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: ItemAddParams = parse(params)?;
    let config = ctx.config.load()?;
    let (now, ids) = (ctx.now(), ctx.ids);
    let today = day::current(&config, now)?;
    mutate(ctx.store, &p.op_id, |tx| {
        let mut new = match (&p.quick_text, &p.title) {
            (Some(text), _) => quick::parse(text)?.into_new_item(),
            (None, Some(title)) => NewItem {
                title: title.clone(),
                ..NewItem::default()
            },
            (None, None) => {
                return Err(Error::validation(
                    "title",
                    "title or quick_text is required",
                ));
            }
        };
        if let (Some(_), Some(title)) = (&p.quick_text, &p.title) {
            new.title = title.clone();
        }
        new.block = p.block.or(new.block);
        new.repeat = p.repeat.or(new.repeat);
        new.weekdays = p.weekdays.or(new.weekdays);
        new.planned_minutes = p.planned_minutes.or(new.planned_minutes);
        new.project_id = p.project_id.clone();
        new.date = p.date;
        let item = items::add(tx, &config, now, ids, new)?;
        Ok(item_views(today, vec![item]))
    })
}

pub(crate) fn item_edit(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: ItemEditParams = parse(params)?;
    let now = ctx.now();
    let today = day::current(&ctx.config.load()?, now)?;
    mutate(ctx.store, &p.op_id, |tx| {
        let item = items::edit(tx, now, &p.id, p.changes.clone(), p.expected_version)?;
        Ok(item_views(today, vec![item]))
    })
}

pub(crate) fn item_move(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: ItemMoveParams = parse(params)?;
    let position = match (&p.index, &p.after_id, &p.before_id) {
        (None, None, None) => MovePosition::Index(0),
        (Some(index), None, None) => MovePosition::Index(*index),
        (None, Some(anchor), None) => MovePosition::After(anchor.clone()),
        (None, None, Some(anchor)) => MovePosition::Before(anchor.clone()),
        _ => {
            return Err(Error::validation(
                "position",
                "use only one of index, after_id, or before_id",
            )
            .into());
        }
    };
    let now = ctx.now();
    let today = day::current(&ctx.config.load()?, now)?;
    mutate(ctx.store, &p.op_id, |tx| {
        let moved = items::move_to(
            tx,
            now,
            &p.id,
            p.block,
            position.clone(),
            p.expected_version,
        )?;
        Ok(item_views(today, moved))
    })
}

pub(crate) fn item_remove(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: RemoveParams = parse(params)?;
    let now = ctx.now();
    mutate(ctx.store, &p.op_id, |tx| {
        Ok(removed(
            items::remove(tx, now, &p.id, p.expected_version)?.id,
        ))
    })
}

pub(crate) fn project_add(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: ProjectAddParams = parse(params)?;
    let (now, ids) = (ctx.now(), ctx.ids);
    mutate(ctx.store, &p.op_id, |tx| {
        let project = projects::add(tx, now, ids, p.project.clone())?;
        project_view(tx, project)
    })
}

pub(crate) fn project_edit(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: ProjectEditParams = parse(params)?;
    let now = ctx.now();
    mutate(ctx.store, &p.op_id, |tx| {
        let project = projects::edit(tx, now, &p.id, p.changes.clone(), p.expected_version)?;
        project_view(tx, project)
    })
}

pub(crate) fn tag_add(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: TagAddParams = parse(params)?;
    let (now, ids) = (ctx.now(), ctx.ids);
    mutate(ctx.store, &p.op_id, |tx| {
        Ok(Changed {
            tags: vec![tags::add(tx, now, ids, &p.name, &p.color)?],
            ..Changed::default()
        })
    })
}

pub(crate) fn tag_edit(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: TagEditParams = parse(params)?;
    let now = ctx.now();
    mutate(ctx.store, &p.op_id, |tx| {
        let tag = tags::edit(
            tx,
            now,
            &p.id,
            p.name.as_deref(),
            p.color.as_deref(),
            p.expected_version,
        )?;
        Ok(Changed {
            tags: vec![tag],
            ..Changed::default()
        })
    })
}

/// Removing a tag also detaches it from its projects, which are returned.
pub(crate) fn tag_remove(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: RemoveParams = parse(params)?;
    let now = ctx.now();
    mutate(ctx.store, &p.op_id, |tx| {
        let tagged: Vec<String> = tx
            .projects()?
            .into_iter()
            .filter(|project| project.tag_ids.contains(&p.id))
            .map(|project| project.id)
            .collect();
        let tag = tags::remove(tx, now, &p.id, p.expected_version)?;
        let projects: Vec<ProjectView> = tagged
            .iter()
            .map(|id| projects::view(tx, projects::find(tx, id)?))
            .collect::<Result<_, Error>>()?;
        Ok(Changed {
            projects,
            ..removed(tag.id)
        })
    })
}

pub(crate) fn metric_add(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: MetricAddParams = parse(params)?;
    let (now, ids) = (ctx.now(), ctx.ids);
    mutate(ctx.store, &p.op_id, |tx| {
        Ok(Changed {
            metrics: vec![metrics::add(tx, now, ids, p.metric.clone())?],
            ..Changed::default()
        })
    })
}

pub(crate) fn metric_edit(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: MetricEditParams = parse(params)?;
    let now = ctx.now();
    mutate(ctx.store, &p.op_id, |tx| {
        let metric = metrics::edit(tx, now, &p.id, p.changes.clone(), p.expected_version)?;
        Ok(Changed {
            metrics: vec![metric],
            ..Changed::default()
        })
    })
}

pub(crate) fn session_start(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: SessionStartParams = parse(params)?;
    let config = ctx.config.load()?;
    let (now, ids) = (ctx.now(), ctx.ids);
    mutate(ctx.store, &p.op_id, |tx| {
        let session = sessions::start(tx, &config, now, ids, p.target.clone())?;
        Ok(session_view(&session))
    })
}

pub(crate) fn session_stop(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: SessionStopParams = parse(params)?;
    let now = ctx.now();
    mutate(ctx.store, &p.op_id, |tx| {
        let session = sessions::stop(tx, now, p.id.as_deref(), p.values.clone(), p.note.clone())?;
        Ok(session_view(&session))
    })
}

pub(crate) fn session_log(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: SessionLogParams = parse(params)?;
    let config = ctx.config.load()?;
    let (now, ids) = (ctx.now(), ctx.ids);
    mutate(ctx.store, &p.op_id, |tx| {
        let session = sessions::log(tx, &config, now, ids, p.log.clone())?;
        Ok(session_view(&session))
    })
}

pub(crate) fn session_edit(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: SessionEditParams = parse(params)?;
    let now = ctx.now();
    mutate(ctx.store, &p.op_id, |tx| {
        let session = sessions::edit(tx, now, &p.id, p.changes.clone(), p.expected_version)?;
        Ok(session_view(&session))
    })
}

pub(crate) fn session_remove(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: RemoveParams = parse(params)?;
    let now = ctx.now();
    mutate(ctx.store, &p.op_id, |tx| {
        Ok(removed(
            sessions::remove(tx, now, &p.id, p.expected_version)?.id,
        ))
    })
}

pub(crate) fn prayer_mark(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    prayer(params, ctx, true)
}

pub(crate) fn prayer_unmark(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    prayer(params, ctx, false)
}

fn prayer(params: &Value, ctx: &mut Context<'_>, prayed: bool) -> Result<Value, RpcError> {
    let p: PrayerParams = parse(params)?;
    let config = ctx.config.load()?;
    let now = ctx.now();
    mutate(ctx.store, &p.op_id, |tx| {
        let mark = marks::set(
            tx,
            &config,
            now,
            p.prayer,
            p.date,
            prayed,
            p.expected_version,
        )?;
        Ok(Changed {
            marks: vec![mark],
            ..Changed::default()
        })
    })
}

/// The config file cannot join the SQLite transaction, so it is written
/// first (by atomic rename) and the op is then recorded in the store. A replay
/// returns the recorded `Config` without touching the file; a crash between
/// the two lets a retry apply its body once more.
pub(crate) fn config_set(params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: ConfigSetParams = parse(params)?;
    if !p.supplied() {
        return Err(
            Error::validation("config", "no settings supplied; set at least one field").into(),
        );
    }
    let config = p.apply(ctx.config.load()?);
    config.validate()?;
    let Some(op_id) = &p.op_id else {
        ctx.config.save(&config)?;
        return to_value(config);
    };
    let repository = ctx.config;
    let stored = ctx.store.write(|tx| {
        tx.set_op_id(Some(op_id));
        if let Some(applied) = tx.applied_op(op_id)? {
            return serde_json::from_str::<Value>(&applied.result_json)
                .map_err(|error| Error::Storage(format!("invalid applied op result: {error}")));
        }
        repository.save(&config)?;
        let result = serde_json::to_value(&config)
            .map_err(|error| Error::Internal(format!("serialize config: {error}")))?;
        tx.record_applied_op(op_id, &result.to_string())?;
        Ok(result)
    })?;
    Ok(stored)
}
