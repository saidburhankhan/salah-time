//! Mutating methods. Each runs in one write transaction that also records its
//! complete result under the client's `op_id`, so a retry returns the same
//! bytes and writes nothing.

use crate::dispatcher::{Call, Context, parse, to_value};
use crate::error::RpcError;
use crate::methods::*;
use chrono::{DateTime, NaiveDate, Utc};
use salah_core::config::Config;
use salah_core::items::{ItemView, MovePosition, NewItem};
use salah_core::projects::ProjectView;
use salah_core::sessions::SessionView;
use salah_core::{Store, WriteTx, day, items, marks, metrics, projects, quick, sessions, tags};
use salah_domain::error::Error;
use salah_domain::tracking::{Item, Project, Session};
use serde::Serialize;
use serde_json::Value;

/// Looks up an operation before parsing its retry body. The writer repeats
/// the lookup inside its transaction in case another caller won the race.
pub(crate) fn replay(store: &Store, method: &str, op_id: &str) -> Result<Option<Value>, Error> {
    if op_id.trim().is_empty() {
        return Err(Error::validation("op_id", "op_id must not be empty"));
    }
    let Some(applied) = store.applied_op(op_id)? else {
        return Ok(None);
    };
    if applied.method != method {
        return Err(Error::validation(
            "op_id",
            format!("op_id {op_id:?} was already used for {}", applied.method),
        ));
    }
    serde_json::from_str(&applied.result_json)
        .map(Some)
        .map_err(|error| Error::Storage(format!("invalid applied op result: {error}")))
}

/// Runs `work` once per `op_id` and stores its result under the op id and
/// `method`. A replay returns the stored result before any validation or
/// write; reusing an op id for another method is a validation error.
fn replayable<T: Serialize>(
    store: &mut Store,
    method: &str,
    op_id: &str,
    mut work: impl FnMut(&WriteTx<'_>) -> Result<T, Error>,
) -> Result<Value, RpcError> {
    let result = store.write(|tx| {
        tx.set_op_id(Some(op_id));
        if let Some(result) = replay(tx, method, op_id)? {
            return Ok(result);
        }
        let result = serde_json::to_value(work(tx)?)
            .map_err(|error| Error::Internal(format!("serialize result: {error}")))?;
        tx.record_applied_op(op_id, method, &result.to_string())?;
        Ok(result)
    })?;
    Ok(result)
}

/// A replayable mutation whose result is `{revision, op_id, changed}`.
fn mutate(
    store: &mut Store,
    method: &str,
    op_id: &str,
    mut work: impl FnMut(&WriteTx<'_>) -> Result<Changed, Error>,
) -> Result<Value, RpcError> {
    replayable(store, method, op_id, |tx| {
        Ok(MutationResult {
            changed: work(tx)?,
            revision: tx.revision()?,
            op_id: op_id.to_owned(),
        })
    })
}

fn item_views(store: &Store, today: NaiveDate, rows: Vec<Item>) -> Result<Changed, Error> {
    Ok(Changed {
        items: rows
            .into_iter()
            .map(|item| ItemView::new(store, item, today))
            .collect::<Result<_, _>>()?,
        ..Changed::default()
    })
}

fn project_view(store: &Store, project: Project) -> Result<Changed, Error> {
    Ok(Changed {
        projects: vec![projects::view(store, project)?],
        ..Changed::default()
    })
}

fn session_views<'a>(
    store: &Store,
    config: &Config,
    now: DateTime<Utc>,
    sessions: impl IntoIterator<Item = &'a Session>,
) -> Result<Changed, Error> {
    Ok(Changed {
        sessions: sessions
            .into_iter()
            .map(|session| SessionView::at(store, config, now, session))
            .collect::<Result<_, _>>()?,
        ..Changed::default()
    })
}

fn removed(id: String) -> Changed {
    Changed {
        deleted: vec![id],
        ..Changed::default()
    }
}

pub(crate) fn item_add(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: ItemAddParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let (now, ids) = (ctx.now(), ctx.ids);
    let today = day::current(&config, now)?;
    mutate(ctx.store, call.method, &p.op_id, |tx| {
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
        new.metric_ids = p.metric_ids.clone();
        new.date = p.date;
        let item = items::add(tx, &config, now, ids, new)?;
        item_views(tx, today, vec![item])
    })
}

pub(crate) fn item_edit(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: ItemEditParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let now = ctx.now();
    let today = day::current(&config, now)?;
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        let item = items::edit(
            tx,
            &config,
            now,
            &p.id,
            p.changes.clone(),
            p.expected_version,
        )?;
        item_views(tx, today, vec![item])
    })
}

pub(crate) fn item_move(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: ItemMoveParams = parse(call.params)?;
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
    let config = ctx.config.load()?;
    let now = ctx.now();
    let today = day::current(&config, now)?;
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        let moved = items::move_to(
            tx,
            &config,
            now,
            &p.id,
            p.block,
            position.clone(),
            p.expected_version,
        )?;
        item_views(tx, today, moved)
    })
}

pub(crate) fn item_remove(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: RemoveParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let now = ctx.now();
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        Ok(removed(
            items::remove(tx, &config, now, &p.id, p.expected_version)?.id,
        ))
    })
}

pub(crate) fn project_add(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: ProjectAddParams = parse(call.params)?;
    let (now, ids) = (ctx.now(), ctx.ids);
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        let project = projects::add(tx, now, ids, p.project.clone())?;
        project_view(tx, project)
    })
}

pub(crate) fn project_edit(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: ProjectEditParams = parse(call.params)?;
    let now = ctx.now();
    let config = ctx.config;
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        let (project, affected) =
            projects::edit(tx, now, &p.id, p.changes.clone(), p.expected_version)?;
        let mut changed = project_view(tx, project)?;
        if !affected.is_empty() {
            let today = day::current(&config.load()?, now)?;
            changed.items = item_views(tx, today, affected)?.items;
        }
        Ok(changed)
    })
}

pub(crate) fn tag_add(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: TagAddParams = parse(call.params)?;
    let (now, ids) = (ctx.now(), ctx.ids);
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        Ok(Changed {
            tags: vec![tags::add(tx, now, ids, &p.name, &p.color)?],
            ..Changed::default()
        })
    })
}

pub(crate) fn tag_edit(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: TagEditParams = parse(call.params)?;
    let now = ctx.now();
    mutate(ctx.store, call.method, &p.op_id, |tx| {
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
pub(crate) fn tag_remove(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: RemoveParams = parse(call.params)?;
    let now = ctx.now();
    mutate(ctx.store, call.method, &p.op_id, |tx| {
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

pub(crate) fn metric_add(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: MetricAddParams = parse(call.params)?;
    let (now, ids) = (ctx.now(), ctx.ids);
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        Ok(Changed {
            metrics: vec![metrics::add(tx, now, ids, p.metric.clone())?],
            ..Changed::default()
        })
    })
}

pub(crate) fn metric_edit(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: MetricEditParams = parse(call.params)?;
    let now = ctx.now();
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        let metric = metrics::edit(tx, now, &p.id, p.changes.clone(), p.expected_version)?;
        Ok(Changed {
            metrics: vec![metric],
            ..Changed::default()
        })
    })
}

pub(crate) fn session_start(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: SessionStartParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let (now, ids) = (ctx.now(), ctx.ids);
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        let session = sessions::start(tx, &config, now, ids, p.target.clone())?;
        session_views(tx, &config, now, [&session])
    })
}

pub(crate) fn session_stop(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: SessionStopParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let (now, ids) = (ctx.now(), ctx.ids);
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        let result = sessions::stop(tx, &config, now, ids, p.id.as_deref(), p.stop.clone())?;
        session_views(
            tx,
            &config,
            now,
            std::iter::once(&result.stopped).chain(&result.tick),
        )
    })
}

pub(crate) fn session_bump(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: SessionBumpParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let now = ctx.now();
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        let session = sessions::bump(tx, &config, now, p.id.as_deref(), &p.metric_id, p.delta)?;
        session_views(tx, &config, now, [&session])
    })
}

pub(crate) fn session_note(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: SessionNoteParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let (now, ids) = (ctx.now(), ctx.ids);
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        let session = sessions::append_note(tx, &config, now, ids, p.id.as_deref(), &p.text)?;
        session_views(tx, &config, now, [&session])
    })
}

pub(crate) fn session_log(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: SessionLogParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let (now, ids) = (ctx.now(), ctx.ids);
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        let result = sessions::log_with_changes(tx, &config, now, ids, p.log.clone())?;
        session_views(
            tx,
            &config,
            now,
            std::iter::once(&result.logged).chain(&result.stopped),
        )
    })
}

pub(crate) fn session_edit(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: SessionEditParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let now = ctx.now();
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        let session = sessions::edit(
            tx,
            &config,
            now,
            &p.id,
            p.changes.clone(),
            p.expected_version,
        )?;
        session_views(tx, &config, now, [&session])
    })
}

pub(crate) fn session_remove(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: RemoveParams = parse(call.params)?;
    let config = ctx.config.load()?;
    let now = ctx.now();
    mutate(ctx.store, call.method, &p.op_id, |tx| {
        Ok(removed(
            sessions::remove(tx, &config, now, &p.id, p.expected_version)?.id,
        ))
    })
}

/// `prayer.mark` and `prayer.unmark`.
pub(crate) fn prayer(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: PrayerParams = parse(call.params)?;
    let prayed = call.method == "prayer.mark";
    let config = ctx.config.load()?;
    let now = ctx.now();
    mutate(ctx.store, call.method, &p.op_id, |tx| {
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
pub(crate) fn config_set(call: Call<'_>, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
    let p: ConfigSetParams = parse(call.params)?;
    if !p.supplied() {
        return Err(
            Error::validation("config", "no settings supplied; set at least one field").into(),
        );
    }
    let repository = ctx.config;
    let now = ctx.now();
    let save = |tx: &WriteTx<'_>| {
        let old = repository.load()?;
        let config = p.apply(old.clone());
        config.validate()?;
        salah_core::config::record_change(tx, now, &old, &config)?;
        repository.save(&config)?;
        Ok(config)
    };
    match &p.op_id {
        Some(id) => replayable(ctx.store, call.method, id, save),
        None => to_value(ctx.store.write(save)?),
    }
}
