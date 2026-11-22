//! The transport-independent dispatcher. It performs no file, socket, or
//! terminal I/O of its own: storage comes through a `Store`, configuration
//! through an injected repository, and time/ids through domain traits.

use crate::error::RpcError;
use crate::message::{MAX_LINE_BYTES, PROTOCOL_VERSION, Request, Response, SERVER_NAME};
use crate::methods::*;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use salah_core::config::Config;
use salah_core::export::{self, ImportSummary};
use salah_core::{Store, WriteTx, planner, quick, status};
use salah_domain::error::Error;
use salah_domain::item::{LegacyEntry, LegacyItem, PrayerMark};
use salah_domain::runtime::{Clock, IdGenerator};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::collections::HashSet;

/// Configuration is I/O; the dispatcher only sees this port.
pub trait ConfigRepository {
    fn load(&self) -> Result<Config, Error>;
    fn save(&self, config: &Config) -> Result<(), Error>;
}

/// Everything a request needs. S4's daemon constructs the same context around
/// its shared store and scheduler clock.
pub struct Context<'a> {
    pub store: &'a mut Store,
    pub config: &'a dyn ConfigRepository,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
}

#[derive(Clone, Debug)]
struct Handshake {
    #[allow(dead_code)]
    protocol: u32,
    #[allow(dead_code)]
    client: String,
    #[allow(dead_code)]
    actor: Option<String>,
}

/// Per-connection protocol state: successful `hello`, then method dispatch.
#[derive(Clone, Debug, Default)]
pub struct Dispatcher {
    hello: Option<Handshake>,
}

impl Dispatcher {
    pub const CAPABILITIES: [&'static str; 3] = ["timer.v1", "events.v1", "clock.v1"];

    /// Methods that mutate the store or config. The daemon routes exactly
    /// these through its single writer thread; everything else is read-only
    /// and may run concurrently on a per-connection store.
    pub const MUTATION_METHODS: [&'static str; 13] = [
        "item.add",
        "item.edit",
        "item.move",
        "item.remove",
        "entry.done",
        "entry.undo",
        "entry.log",
        "timer.start",
        "timer.stop",
        "prayer.mark",
        "prayer.unmark",
        "config.set",
        "import",
    ];

    pub fn is_mutation(method: &str) -> bool {
        Self::MUTATION_METHODS.contains(&method)
    }

    pub fn new() -> Self {
        Self::default()
    }

    /// True after a successful `hello` on this dispatcher.
    pub fn hello_done(&self) -> bool {
        self.hello.is_some()
    }

    /// Marks the handshake as complete without performing it. The daemon's
    /// writer dispatcher receives requests from connections that have already
    /// passed their own handshake.
    pub fn accept_hello(&mut self) {
        self.hello = Some(Handshake {
            protocol: PROTOCOL_VERSION,
            client: "daemon".into(),
            actor: None,
        });
    }

    /// Parses one NDJSON line and returns the response line, if any. Blank
    /// lines are ignored. The caller owns framing and newline handling.
    pub fn handle_line(&mut self, line: &str, ctx: &mut Context<'_>) -> Option<Response> {
        if line.trim().is_empty() {
            return None;
        }
        if line.len() > MAX_LINE_BYTES {
            return Some(Response::err(
                Value::Null,
                RpcError::parse(format!(
                    "request line exceeds the {MAX_LINE_BYTES}-byte limit"
                )),
            ));
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(error) => {
                return Some(Response::err(
                    Value::Null,
                    RpcError::parse(format!("invalid JSON: {error}")),
                ));
            }
        };
        Some(self.handle_value(value, ctx))
    }

    /// Dispatches a parsed JSON value. Invalid envelopes still produce a
    /// response with `id: null` when the id cannot be read.
    pub fn handle_value(&mut self, value: Value, ctx: &mut Context<'_>) -> Response {
        let id = value.get("id").cloned().unwrap_or(Value::Null);
        let request = match Request::parse(value) {
            Ok(request) => request,
            Err(error) => return Response::err(id, error),
        };
        self.dispatch(&request, ctx)
    }

    pub fn dispatch(&mut self, request: &Request, ctx: &mut Context<'_>) -> Response {
        match self.dispatch_result(request, ctx) {
            Ok(result) => Response::ok(request.id.clone(), result),
            Err(error) => Response::err(request.id.clone(), error),
        }
    }

    fn dispatch_result(
        &mut self,
        request: &Request,
        ctx: &mut Context<'_>,
    ) -> Result<Value, RpcError> {
        if request.method != "hello" && self.hello.is_none() {
            return Err(RpcError::invalid_request(
                "hello must be the first request on a connection",
            ));
        }
        let params = &request.params;
        match request.method.as_str() {
            "hello" => self.hello(params, ctx),
            "status.get" => self.status_get(params, ctx),
            "day.get" => self.day_get(params, ctx),
            "items.list" => self.items_list(params, ctx),
            "stats.get" => self.stats_get(params, ctx),
            "history.get" => self.history_get(params, ctx),
            "item.parse" => self.item_parse(params, ctx),
            "item.preview" => self.item_preview(params, ctx),
            "item.add" => self.item_add(params, ctx),
            "item.edit" => self.item_edit(params, ctx),
            "item.move" => self.item_move(params, ctx),
            "item.remove" => self.item_remove(params, ctx),
            "entry.done" => self.entry_done(params, ctx),
            "entry.undo" => self.entry_undo(params, ctx),
            "entry.log" => self.entry_log(params, ctx),
            "timer.start" => self.timer_start(params, ctx),
            "timer.stop" => self.timer_stop(params, ctx),
            "prayer.mark" => self.prayer(params, ctx, true),
            "prayer.unmark" => self.prayer(params, ctx, false),
            "config.get" => self.config_get(params, ctx),
            "config.set" => self.config_set(params, ctx),
            "export" => self.export(params, ctx),
            "import" => self.import(params, ctx),
            "subscribe" => self.subscribe(params, ctx),
            method => Err(RpcError::method_not_found(method)),
        }
    }

    // -- handshake ---------------------------------------------------------

    fn hello(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: HelloParams = parse_params(params)?;
        if !params.protocol.contains(&PROTOCOL_VERSION) {
            return Err(RpcError::invalid_params(format!(
                "unsupported protocol version(s) {:?}; server supports {}",
                params.protocol, PROTOCOL_VERSION
            )));
        }
        self.hello = Some(Handshake {
            protocol: PROTOCOL_VERSION,
            client: params.client,
            actor: params.actor,
        });
        to_value(HelloResult {
            protocol: PROTOCOL_VERSION,
            server: SERVER_NAME.to_string(),
            capabilities: Self::CAPABILITIES
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            revision: ctx.store.revision()?,
        })
    }

    // -- reads -------------------------------------------------------------

    fn status_get(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        parse_empty(params)?;
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        let mut report = status::build(&config, now)?;
        report.attach_planner(planner::today(ctx.store, &config, now));
        to_value(StatusView::from(report))
    }

    fn day_get(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: DayParams = parse_params(params)?;
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        to_value(DayPlanView::from(planner::day_plan(
            ctx.store,
            &config,
            now,
            params.date,
        )?))
    }

    fn items_list(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        parse_empty(params)?;
        let items = ctx
            .store
            .legacy_items()?
            .into_iter()
            .filter(LegacyItem::is_live)
            .map(|item| ItemRecord {
                version: item.version,
                item,
            })
            .collect::<Vec<_>>();
        to_value(items)
    }

    fn stats_get(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: StatsParams = parse_params(params)?;
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        let (first, last) = match (params.from, params.to, params.days) {
            (Some(from), Some(to), _) => (from, to),
            (None, None, days) => {
                let days = days.unwrap_or(30);
                if !(1..=planner::MAX_HISTORY_DAYS).contains(&days) {
                    return Err(Error::validation(
                        "days",
                        format!("days must be between 1 and {}", planner::MAX_HISTORY_DAYS),
                    )
                    .into());
                }
                let last = current_day(&config, now)?;
                (last - Duration::days(days - 1), last)
            }
            _ => {
                return Err(
                    Error::validation("from", "from and to must be supplied together").into(),
                );
            }
        };
        to_value(StatsView::from(planner::stats(
            ctx.store, &config, now, first, last,
        )?))
    }

    fn history_get(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: HistoryParams = parse_params(params)?;
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        let last = params.to.map_or_else(|| current_day(&config, now), Ok)?;
        let first = params.from.unwrap_or(last - Duration::days(6));
        to_value(planner::history(ctx.store, &config, now, first, last)?)
    }

    fn item_parse(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: ParseParams = parse_params(params)?;
        let _ = ctx;
        to_value(quick::parse(&params.text)?)
    }

    /// Recurrence preview for the entry form. Read-only: no store access and
    /// no `op_id`.
    fn item_preview(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: PreviewParams = parse_params(params)?;
        let _ = ctx;
        to_value(planner::preview(
            params.block,
            params.repeat,
            params.weekdays,
            params.measure,
            params.target,
            params.date,
        )?)
    }

    // -- item mutations ----------------------------------------------------

    fn item_add(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: ItemAddParams = parse_params(params)?;
        if let Some(stored) = applied_rpc(ctx.store, &params.op_id)? {
            return Ok(stored);
        }
        let mut request = match (&params.quick_text, &params.title) {
            (Some(text), _) => {
                let mut request = quick::parse(text)?.into_new_item();
                if let Some(title) = &params.title {
                    request.title = title.clone();
                }
                request
            }
            (None, Some(title)) => planner::NewItem {
                title: title.clone(),
                ..planner::NewItem::default()
            },
            (None, None) => {
                return Err(Error::validation("title", "title or quick_text is required").into());
            }
        };
        if params.block.is_some() {
            request.block = params.block;
        }
        if params.repeat.is_some() {
            request.repeat = params.repeat;
        }
        if params.weekdays.is_some() {
            request.weekdays = params.weekdays;
        }
        if params.measure.is_some() {
            request.measure = params.measure;
        }
        if params.target.is_some() {
            request.target = params.target;
        }
        request.date = params.date;
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        let mut recorder = |tx: &WriteTx<'_>, stored: &str| -> Result<Value, Error> {
            let item: LegacyItem = decode_result(stored)?;
            let rows = find_item_rows(
                tx,
                &config,
                now,
                std::slice::from_ref(&item.id),
                Some(item.day),
            )?;
            result_value(MutationResult {
                revision: tx.revision()?,
                op_id: params.op_id.clone(),
                changed: Changed {
                    items: rows,
                    ..Changed::default()
                },
                undone: None,
            })
        };
        planner::add_with(
            ctx.store,
            &config,
            now,
            ctx.ids,
            request,
            planner::Mutation::with_result(Some(&params.op_id), None, &mut recorder),
        )?;
        stored_rpc_result(ctx.store, &params.op_id).map_err(Into::into)
    }

    fn item_edit(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: ItemEditParams = parse_params(params)?;
        if let Some(stored) = applied_rpc(ctx.store, &params.op_id)? {
            return Ok(stored);
        }
        ensure_full_item(ctx.store, &params.id)?;
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        let mut recorder = |tx: &WriteTx<'_>, stored: &str| -> Result<Value, Error> {
            let item: LegacyItem = decode_result(stored)?;
            let rows = find_item_rows(
                tx,
                &config,
                now,
                std::slice::from_ref(&item.id),
                Some(item.day),
            )?;
            result_value(MutationResult {
                revision: tx.revision()?,
                op_id: params.op_id.clone(),
                changed: Changed {
                    items: rows,
                    ..Changed::default()
                },
                undone: None,
            })
        };
        planner::edit_with(
            ctx.store,
            &config,
            now,
            &params.id,
            params.changes(),
            planner::Mutation::with_result(
                Some(&params.op_id),
                params.expected_version,
                &mut recorder,
            ),
        )?;
        stored_rpc_result(ctx.store, &params.op_id).map_err(Into::into)
    }

    fn item_move(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: ItemMoveParams = parse_params(params)?;
        if let Some(stored) = applied_rpc(ctx.store, &params.op_id)? {
            return Ok(stored);
        }
        let selectors = [
            params.index.is_some(),
            params.after_id.is_some(),
            params.before_id.is_some(),
        ]
        .into_iter()
        .filter(|set| *set)
        .count();
        if selectors > 1 {
            return Err(Error::validation(
                "position",
                "use only one of index, after_id, or before_id",
            )
            .into());
        }
        ensure_full_item(ctx.store, &params.id)?;
        let position = match (&params.after_id, &params.before_id, params.index) {
            (Some(anchor), _, _) => planner::MovePosition::After(anchor.clone()),
            (_, Some(anchor), _) => planner::MovePosition::Before(anchor.clone()),
            (_, _, Some(index)) => planner::MovePosition::Index(index),
            (None, None, None) => planner::MovePosition::Index(0),
        };
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        let block = params.block;
        let mut recorder = |tx: &WriteTx<'_>, stored: &str| -> Result<Value, Error> {
            let item: LegacyItem = decode_result(stored)?;
            let changed_ids: Vec<String> = tx
                .legacy_items()?
                .into_iter()
                .filter(|item| item.is_live() && item.block == block)
                .map(|item| item.id)
                .collect();
            let rows = find_item_rows(tx, &config, now, &changed_ids, Some(item.day))?;
            result_value(MutationResult {
                revision: tx.revision()?,
                op_id: params.op_id.clone(),
                changed: Changed {
                    items: rows,
                    ..Changed::default()
                },
                undone: None,
            })
        };
        planner::move_item_with(
            ctx.store,
            &config,
            now,
            &params.id,
            block,
            position,
            planner::Mutation::with_result(
                Some(&params.op_id),
                params.expected_version,
                &mut recorder,
            ),
        )?;
        stored_rpc_result(ctx.store, &params.op_id).map_err(Into::into)
    }

    fn item_remove(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: ItemRemoveParams = parse_params(params)?;
        if let Some(stored) = applied_rpc(ctx.store, &params.op_id)? {
            return Ok(stored);
        }
        ensure_full_item(ctx.store, &params.id)?;
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        let mut recorder = |tx: &WriteTx<'_>, stored: &str| -> Result<Value, Error> {
            let item: LegacyItem = decode_result(stored)?;
            result_value(MutationResult {
                revision: tx.revision()?,
                op_id: params.op_id.clone(),
                changed: Changed {
                    deleted: vec![item.id.clone()],
                    ..Changed::default()
                },
                undone: None,
            })
        };
        planner::remove_with(
            ctx.store,
            &config,
            now,
            &params.id,
            planner::Mutation::with_result(
                Some(&params.op_id),
                params.expected_version,
                &mut recorder,
            ),
        )?;
        stored_rpc_result(ctx.store, &params.op_id).map_err(Into::into)
    }

    // -- entry mutations ---------------------------------------------------

    fn entry_done(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: EntryDoneParams = parse_params(params)?;
        if let Some(stored) = applied_rpc(ctx.store, &params.op_id)? {
            return Ok(stored);
        }
        ensure_full_item(ctx.store, &params.item_id)?;
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        let date = params.date;
        let mut recorder = |tx: &WriteTx<'_>, stored: &str| -> Result<Value, Error> {
            let entry: LegacyEntry = decode_result(stored)?;
            let day = date.unwrap_or(entry.block.day);
            let rows = find_item_rows(
                tx,
                &config,
                now,
                std::slice::from_ref(&params.item_id),
                Some(day),
            )?;
            result_value(MutationResult {
                revision: tx.revision()?,
                op_id: params.op_id.clone(),
                changed: Changed {
                    items: rows,
                    entries: vec![entry_record(&entry)],
                    ..Changed::default()
                },
                undone: None,
            })
        };
        planner::done_with(
            ctx.store,
            &config,
            now,
            ctx.ids,
            &params.item_id,
            date,
            planner::Mutation::with_result(Some(&params.op_id), None, &mut recorder),
        )?;
        stored_rpc_result(ctx.store, &params.op_id).map_err(Into::into)
    }

    fn entry_undo(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: EntryUndoParams = parse_params(params)?;
        if let Some(stored) = applied_rpc(ctx.store, &params.op_id)? {
            return Ok(stored);
        }
        ensure_full_item(ctx.store, &params.item_id)?;
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        let date = params.date;
        let mut recorder = |tx: &WriteTx<'_>, stored: &str| -> Result<Value, Error> {
            let undone: usize = decode_result(stored)?;
            let day = match date {
                Some(day) => day,
                None => current_day(&config, now)?,
            };
            let rows = find_item_rows(
                tx,
                &config,
                now,
                std::slice::from_ref(&params.item_id),
                Some(day),
            )?;
            result_value(MutationResult {
                revision: tx.revision()?,
                op_id: params.op_id.clone(),
                changed: Changed {
                    items: rows,
                    ..Changed::default()
                },
                undone: Some(undone),
            })
        };
        planner::undo_with(
            ctx.store,
            &config,
            now,
            &params.item_id,
            date,
            planner::Mutation::with_result(Some(&params.op_id), None, &mut recorder),
        )?;
        stored_rpc_result(ctx.store, &params.op_id).map_err(Into::into)
    }

    fn entry_log(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: EntryLogParams = parse_params(params)?;
        if let Some(stored) = applied_rpc(ctx.store, &params.op_id)? {
            return Ok(stored);
        }
        ensure_full_item(ctx.store, &params.item_id)?;
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        let date = params.date;
        let value = params.value;
        let kind = params.kind;
        let mut recorder = |tx: &WriteTx<'_>, stored: &str| -> Result<Value, Error> {
            let entry: LegacyEntry = decode_result(stored)?;
            let day = date.unwrap_or(entry.block.day);
            let rows = find_item_rows(
                tx,
                &config,
                now,
                std::slice::from_ref(&params.item_id),
                Some(day),
            )?;
            result_value(MutationResult {
                revision: tx.revision()?,
                op_id: params.op_id.clone(),
                changed: Changed {
                    items: rows,
                    entries: vec![entry_record(&entry)],
                    ..Changed::default()
                },
                undone: None,
            })
        };
        let mutation = planner::Mutation::with_result(Some(&params.op_id), None, &mut recorder);
        match kind {
            Some(LogKind::Minutes) => planner::adjust_time_with(
                ctx.store,
                &config,
                now,
                ctx.ids,
                &params.item_id,
                value,
                date,
                mutation,
            )?,
            Some(LogKind::Count) => planner::log_count_with(
                ctx.store,
                &config,
                now,
                ctx.ids,
                &params.item_id,
                value,
                date,
                mutation,
            )?,
            None => planner::log_with(
                ctx.store,
                &config,
                now,
                ctx.ids,
                &params.item_id,
                value,
                date,
                mutation,
            )?,
        };
        stored_rpc_result(ctx.store, &params.op_id).map_err(Into::into)
    }

    // -- timer mutations ---------------------------------------------------

    fn timer_start(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: TimerStartParams = parse_params(params)?;
        if let Some(stored) = applied_rpc(ctx.store, &params.op_id)? {
            return Ok(stored);
        }
        ensure_full_item(ctx.store, &params.item_id)?;
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        let mut recorder = |tx: &WriteTx<'_>, stored: &str| -> Result<Value, Error> {
            let started: planner::StartedTimer = decode_result(stored)?;
            let day = current_day(&config, now)?;
            let mut ids = Vec::new();
            let mut seen = HashSet::new();
            if seen.insert(started.timer.item_id.clone()) {
                ids.push(started.timer.item_id.clone());
            }
            for entry in &started.stopped {
                if seen.insert(entry.item_id.clone()) {
                    ids.push(entry.item_id.clone());
                }
            }
            let rows = find_item_rows(tx, &config, now, &ids, Some(day))?;
            result_value(MutationResult {
                revision: tx.revision()?,
                op_id: params.op_id.clone(),
                changed: Changed {
                    items: rows,
                    entries: vec![entry_record(&started.timer)],
                    ..Changed::default()
                },
                undone: None,
            })
        };
        planner::start_timer_with_stopped(
            ctx.store,
            &config,
            now,
            ctx.ids,
            &params.item_id,
            planner::Mutation::with_result(Some(&params.op_id), None, &mut recorder),
        )?;
        stored_rpc_result(ctx.store, &params.op_id).map_err(Into::into)
    }

    fn timer_stop(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: TimerStopParams = parse_params(params)?;
        if let Some(stored) = applied_rpc(ctx.store, &params.op_id)? {
            return Ok(stored);
        }
        if let Some(item_id) = &params.item_id {
            ensure_full_item(ctx.store, item_id)?;
        }
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        let mut recorder = |tx: &WriteTx<'_>, stored: &str| -> Result<Value, Error> {
            let entries: Vec<LegacyEntry> = decode_result(stored)?;
            let day = current_day(&config, now)?;
            let mut ids = Vec::new();
            let mut seen = HashSet::new();
            for entry in &entries {
                if seen.insert(entry.item_id.clone()) {
                    ids.push(entry.item_id.clone());
                }
            }
            let rows = find_item_rows(tx, &config, now, &ids, Some(day))?;
            result_value(MutationResult {
                revision: tx.revision()?,
                op_id: params.op_id.clone(),
                changed: Changed {
                    items: rows,
                    entries: entries.iter().map(entry_record).collect(),
                    ..Changed::default()
                },
                undone: None,
            })
        };
        let mutation = planner::Mutation::with_result(Some(&params.op_id), None, &mut recorder);
        match &params.item_id {
            Some(item_id) => {
                planner::stop_item_timer_with(ctx.store, &config, now, item_id, mutation)?
            }
            None => planner::stop_timer_with(ctx.store, &config, now, mutation)?,
        };
        stored_rpc_result(ctx.store, &params.op_id).map_err(Into::into)
    }

    // -- prayer mutations --------------------------------------------------

    fn prayer(
        &mut self,
        params: &Value,
        ctx: &mut Context<'_>,
        prayed: bool,
    ) -> Result<Value, RpcError> {
        let params: PrayerParams = parse_params(params)?;
        if let Some(stored) = applied_rpc(ctx.store, &params.op_id)? {
            return Ok(stored);
        }
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        let mut recorder = |tx: &WriteTx<'_>, stored: &str| -> Result<Value, Error> {
            let mark: PrayerMark = decode_result(stored)?;
            result_value(MutationResult {
                revision: tx.revision()?,
                op_id: params.op_id.clone(),
                changed: Changed {
                    marks: vec![mark_record(&mark)],
                    ..Changed::default()
                },
                undone: None,
            })
        };
        let mutation = planner::Mutation::with_result(
            Some(&params.op_id),
            params.expected_version,
            &mut recorder,
        );
        if prayed {
            planner::mark_with(
                ctx.store,
                &config,
                now,
                params.prayer,
                params.date,
                mutation,
            )?;
        } else {
            planner::unmark_with(
                ctx.store,
                &config,
                now,
                params.prayer,
                params.date,
                mutation,
            )?;
        }
        stored_rpc_result(ctx.store, &params.op_id).map_err(Into::into)
    }

    // -- config ------------------------------------------------------------

    fn config_get(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        parse_empty(params)?;
        to_value(ctx.config.load()?)
    }

    fn config_set(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: ConfigSetParams = parse_params(params)?;
        if !params.supplied() {
            return Err(Error::validation(
                "config",
                "no settings supplied; set at least one field",
            )
            .into());
        }
        let config = params.apply(ctx.config.load()?);
        config.validate()?;
        let stored = serde_json::to_string(&config)
            .map_err(|error| Error::Internal(format!("store: {error}")))?;
        let Some(op_id) = &params.op_id else {
            ctx.config.save(&config)?;
            return to_value(config);
        };
        // The config file cannot join the SQLite transaction. Write it first
        // through the repository's atomic rename, then record the op in the
        // same write transaction; a replay therefore never applies a second
        // body. A crash between the rename and the commit leaves the new file
        // without an op row, so a retry applies its body once more.
        let result = ctx.store.write(|tx| {
            tx.set_op_id(Some(op_id));
            if let Some(applied) = tx.applied_op(op_id)? {
                return serde_json::from_str::<Config>(&applied.result_json).map_err(|error| {
                    Error::Storage(format!("invalid applied op result: {error}"))
                });
            }
            ctx.config.save(&config)?;
            tx.record_applied_op(op_id, &stored)?;
            Ok(config.clone())
        })?;
        to_value(result)
    }

    /// The stdio transport cannot push notifications, so `subscribe` only
    /// validates the topic and returns the current revision. The socket daemon
    /// intercepts this method to register the connection for pushes.
    fn subscribe(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: SubscribeParams = parse_params(params)?;
        for topic in &params.topics {
            if topic != "changes" && topic != "clock" {
                return Err(Error::validation(
                    "topics",
                    format!("unknown topic {topic:?}; supported: changes, clock"),
                )
                .into());
            }
        }
        to_value(SubscribeResult {
            revision: ctx.store.revision()?,
            after_revision: params.after_revision,
        })
    }

    fn export(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        parse_empty(params)?;
        let now = ctx.clock.now();
        to_value(export::export(ctx.store, now)?)
    }

    fn import(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: ImportParams = parse_params(params)?;
        export::validate(&params.data)?;
        if let Some(op_id) = &params.op_id
            && let Some(applied) = ctx.store.applied_op(op_id)?
        {
            let summary: ImportSummary = serde_json::from_str(&applied.result_json)
                .map_err(|error| Error::Storage(format!("invalid applied op result: {error}")))?;
            return to_value(ImportResult {
                revision: applied.revision,
                summary,
            });
        }
        let config = ctx.config.load()?;
        let now = ctx.clock.now();
        let summary = ctx.store.write(|tx| {
            tx.set_op_id(params.op_id.as_deref());
            planner::close_expired_timers(tx, &config, now)?;
            let summary = export::import_tx(tx, &params.data)?;
            if let Some(op_id) = &params.op_id {
                let stored = serde_json::to_string(&summary)
                    .map_err(|error| Error::Internal(format!("store: {error}")))?;
                tx.record_applied_op(op_id, &stored)?;
            }
            Ok(summary)
        })?;
        let revision = match &params.op_id {
            Some(op_id) => revision_for(ctx.store, op_id)?,
            None => ctx.store.revision()?,
        };
        to_value(ImportResult { revision, summary })
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn parse_params<T: DeserializeOwned>(params: &Value) -> Result<T, RpcError> {
    let value = if params.is_null() {
        Value::Object(Map::new())
    } else if params.is_object() {
        params.clone()
    } else {
        return Err(RpcError::invalid_params(
            "params must be a JSON object or omitted",
        ));
    };
    serde_json::from_value(value)
        .map_err(|error| RpcError::invalid_params(format!("invalid params: {error}")))
}

fn parse_empty(params: &Value) -> Result<(), RpcError> {
    if params.is_null() {
        return Ok(());
    }
    match params.as_object() {
        Some(object) if object.is_empty() => Ok(()),
        Some(_) => Err(RpcError::invalid_params("this method takes no parameters")),
        None => Err(RpcError::invalid_params(
            "params must be an object or omitted",
        )),
    }
}

fn to_value<T: Serialize>(value: T) -> Result<Value, RpcError> {
    serde_json::to_value(value)
        .map_err(|error| RpcError::internal(format!("serialize result: {error}")))
}

fn current_day(config: &Config, now: DateTime<Utc>) -> Result<NaiveDate, Error> {
    let report = status::build(config, now)?;
    NaiveDate::parse_from_str(&report.day, "%Y-%m-%d")
        .map_err(|error| Error::Internal(format!("status day: {error}")))
}

fn ensure_full_item(store: &Store, id: &str) -> Result<LegacyItem, Error> {
    store
        .legacy_item(id)?
        .filter(LegacyItem::is_live)
        .ok_or_else(|| Error::NotFound {
            what: "item",
            id: id.to_string(),
        })
}

fn revision_for(store: &Store, op_id: &str) -> Result<i64, Error> {
    match store.applied_op(op_id)? {
        Some(applied) => Ok(applied.revision),
        None => store.revision(),
    }
}

/// Serializes a full mutation result while its transaction is open.
fn result_value<T: Serialize>(value: T) -> Result<Value, Error> {
    serde_json::to_value(value)
        .map_err(|error| Error::Internal(format!("serialize result: {error}")))
}

/// Decodes the planner result stored next to the full protocol result.
fn decode_result<T: DeserializeOwned>(stored: &str) -> Result<T, Error> {
    serde_json::from_str(stored)
        .map_err(|error| Error::Storage(format!("invalid applied op result: {error}")))
}

/// Returns the exact protocol result recorded with a first application of
/// `op_id`, if one exists. Older planner-only rows have no full result.
fn applied_rpc(store: &Store, op_id: &str) -> Result<Option<Value>, Error> {
    let Some(applied) = store.applied_op(op_id)? else {
        return Ok(None);
    };
    let stored: Value = serde_json::from_str(&applied.result_json)
        .map_err(|error| Error::Storage(format!("invalid applied op result: {error}")))?;
    Ok(stored.get("rpc").cloned())
}

/// Reads the full result stored in the same transaction as the mutation.
fn stored_rpc_result(store: &Store, op_id: &str) -> Result<Value, Error> {
    applied_rpc(store, op_id)?.ok_or_else(|| {
        Error::Storage(format!(
            "applied op {op_id:?} has no stored protocol result"
        ))
    })
}

fn entry_record(entry: &LegacyEntry) -> EntryRecord {
    EntryRecord {
        version: entry.version,
        entry: entry.clone(),
    }
}

fn mark_record(mark: &PrayerMark) -> MarkRecord {
    MarkRecord {
        version: mark.version,
        mark: mark.clone(),
    }
}

/// Finds the canonical `DayItem` rows for `ids`, trying the preferred day,
/// today, and each item's own anchor day so a mutation on another date still
/// returns its display row.
fn find_item_rows(
    store: &Store,
    config: &Config,
    now: DateTime<Utc>,
    ids: &[String],
    preferred: Option<NaiveDate>,
) -> Result<Vec<ItemRow>, Error> {
    let wanted: HashSet<&str> = ids.iter().map(String::as_str).collect();
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let mut days = Vec::new();
    let mut push_day = |day: NaiveDate| {
        if !days.contains(&day) {
            days.push(day);
        }
    };
    if let Some(day) = preferred {
        push_day(day);
    }
    if let Ok(day) = current_day(config, now) {
        push_day(day);
    }
    for id in ids {
        if let Some(item) = store.legacy_item(id)? {
            push_day(item.day);
        }
    }
    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    for day in days {
        let plan = planner::day_plan(store, config, now, Some(day))?;
        for block in &plan.blocks {
            for row in &block.items {
                if wanted.contains(row.id.as_str()) && seen.insert(row.id.clone()) {
                    rows.push(ItemRow::from(row));
                }
            }
        }
    }
    Ok(rows)
}
