//! The transport-independent dispatcher: framing, the `hello` handshake, and
//! the method table. It performs no file, socket, or terminal I/O: storage
//! comes through a `Store`, configuration through a repository port, and
//! time and ids through domain traits.

use crate::error::RpcError;
use crate::message::{MAX_LINE_BYTES, PROTOCOL_VERSION, Request, Response, SERVER_NAME};
use crate::methods::{HelloParams, HelloResult};
use crate::{reads, resources, writes};
use chrono::{DateTime, Utc};
use salah_core::{Store, config::Config};
use salah_domain::error::Error;
use salah_domain::runtime::{Clock, IdGenerator};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

/// Configuration is I/O; the dispatcher only sees this port.
pub trait ConfigRepository {
    fn load(&self) -> Result<Config, Error>;
    fn save(&self, config: &Config) -> Result<(), Error>;
}

/// Everything a request needs.
pub struct Context<'a> {
    pub store: &'a mut Store,
    pub config: &'a dyn ConfigRepository,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
}

impl Context<'_> {
    /// The request instant, at the store's millisecond precision.
    pub fn now(&self) -> DateTime<Utc> {
        salah_core::stamp(self.clock.now())
    }
}

/// One request as a handler sees it.
#[derive(Clone, Copy)]
pub(crate) struct Call<'a> {
    pub method: &'static str,
    pub params: &'a Value,
}

type Handler = fn(Call<'_>, &mut Context<'_>) -> Result<Value, RpcError>;

/// Whether a method writes. The daemon routes writes through its single
/// writer thread; reads run on a per-connection store.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Read,
    Write,
}

/// Every method after `hello`.
const METHODS: &[(&str, Kind, Handler)] = &[
    ("status.get", Kind::Read, reads::status),
    ("day.get", Kind::Read, reads::day),
    ("items.list", Kind::Read, reads::items),
    ("backlog.list", Kind::Read, reads::backlog),
    ("resources.list", Kind::Read, resources::list),
    ("stats.get", Kind::Read, reads::stats),
    ("stats.table", Kind::Read, reads::stats_table),
    ("stats.series", Kind::Read, reads::stats_series),
    ("sessions.list", Kind::Read, reads::sessions_list),
    ("item.parse", Kind::Read, reads::item_parse),
    ("item.preview", Kind::Read, reads::item_preview),
    ("epics.list", Kind::Read, reads::epics),
    ("projects.list", Kind::Read, reads::projects),
    ("tags.list", Kind::Read, reads::tags),
    ("metrics.list", Kind::Read, reads::metrics),
    ("config.get", Kind::Read, reads::config),
    ("subscribe", Kind::Read, reads::subscribe),
    ("item.add", Kind::Write, writes::item_add),
    ("item.edit", Kind::Write, writes::item_edit),
    ("item.move", Kind::Write, writes::item_move),
    ("item.remove", Kind::Write, writes::item_remove),
    ("item.schedule", Kind::Write, writes::item_schedule),
    ("item.backlog", Kind::Write, writes::item_backlog),
    ("resource.add", Kind::Write, resources::add),
    ("resource.edit", Kind::Write, resources::edit),
    ("resource.remove", Kind::Write, resources::remove),
    ("epic.add", Kind::Write, writes::epic_add),
    ("epic.edit", Kind::Write, writes::epic_edit),
    ("epic.remove", Kind::Write, writes::epic_remove),
    ("project.add", Kind::Write, writes::project_add),
    ("project.edit", Kind::Write, writes::project_edit),
    ("tag.add", Kind::Write, writes::tag_add),
    ("tag.edit", Kind::Write, writes::tag_edit),
    ("tag.remove", Kind::Write, writes::tag_remove),
    ("metric.add", Kind::Write, writes::metric_add),
    ("metric.edit", Kind::Write, writes::metric_edit),
    ("session.start", Kind::Write, writes::session_start),
    ("session.stop", Kind::Write, writes::session_stop),
    ("session.bump", Kind::Write, writes::session_bump),
    ("session.note", Kind::Write, writes::session_note),
    ("session.log", Kind::Write, writes::session_log),
    ("session.edit", Kind::Write, writes::session_edit),
    ("session.remove", Kind::Write, writes::session_remove),
    ("prayer.mark", Kind::Write, writes::prayer),
    ("prayer.unmark", Kind::Write, writes::prayer),
    ("config.set", Kind::Write, writes::config_set),
];

/// Per-connection protocol state: a successful `hello`, then dispatch.
#[derive(Clone, Debug, Default)]
pub struct Dispatcher {
    hello_done: bool,
}

impl Dispatcher {
    pub const CAPABILITIES: [&'static str; 3] = ["tracking.v1", "events.v1", "clock.v1"];

    pub fn new() -> Self {
        Self::default()
    }

    /// Every method name, `hello` first, in table order.
    pub fn methods() -> impl Iterator<Item = &'static str> {
        std::iter::once("hello").chain(METHODS.iter().map(|(name, _, _)| *name))
    }

    /// Methods that mutate the store or config.
    pub fn is_mutation(method: &str) -> bool {
        METHODS
            .iter()
            .any(|(name, kind, _)| *name == method && *kind == Kind::Write)
    }

    pub fn hello_done(&self) -> bool {
        self.hello_done
    }

    /// Marks the handshake done without performing it, for a dispatcher that
    /// serves connections which already shook hands (the daemon's writer, the
    /// in-process CLI).
    pub fn accept_hello(&mut self) {
        self.hello_done = true;
    }

    /// Parses one NDJSON line and returns its response. Blank lines are
    /// ignored; the caller owns framing and newlines.
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
        match serde_json::from_str(line) {
            Ok(value) => Some(self.handle_value(value, ctx)),
            Err(error) => Some(Response::err(
                Value::Null,
                RpcError::parse(format!("invalid JSON: {error}")),
            )),
        }
    }

    /// Dispatches a parsed value; an unreadable envelope answers `id: null`.
    pub fn handle_value(&mut self, value: Value, ctx: &mut Context<'_>) -> Response {
        let id = value.get("id").cloned().unwrap_or(Value::Null);
        match Request::parse(value) {
            Ok(request) => self.dispatch(&request, ctx),
            Err(error) => Response::err(id, error),
        }
    }

    pub fn dispatch(&mut self, request: &Request, ctx: &mut Context<'_>) -> Response {
        match self.call(&request.method, &request.params, ctx) {
            Ok(result) => Response::ok(request.id.clone(), result),
            Err(error) => Response::err(request.id.clone(), error),
        }
    }

    /// Runs one method and returns its bare result.
    pub fn call(
        &mut self,
        method: &str,
        params: &Value,
        ctx: &mut Context<'_>,
    ) -> Result<Value, RpcError> {
        if method == "hello" {
            return self.hello(params, ctx);
        }
        if !self.hello_done {
            return Err(RpcError::invalid_request(
                "hello must be the first request on a connection",
            ));
        }
        let (method, kind, handler) = METHODS
            .iter()
            .find(|(name, _, _)| *name == method)
            .ok_or_else(|| RpcError::method_not_found(method))?;
        if !matches!(params, Value::Null | Value::Object(_)) {
            return Err(RpcError::invalid_params(
                "params must be a JSON object or omitted",
            ));
        }
        if *kind == Kind::Write
            && let Some(op_id) = params.get("op_id").and_then(Value::as_str)
            && let Some(result) = writes::replay(ctx.store, method, op_id)?
        {
            return Ok(result);
        }
        handler(Call { method, params }, ctx)
    }

    fn hello(&mut self, params: &Value, ctx: &mut Context<'_>) -> Result<Value, RpcError> {
        let params: HelloParams = parse(params)?;
        if !params.protocol.contains(&PROTOCOL_VERSION) {
            return Err(RpcError::invalid_params(format!(
                "unsupported protocol version(s) {:?}; server supports {PROTOCOL_VERSION}",
                params.protocol
            )));
        }
        self.hello_done = true;
        to_value(HelloResult {
            protocol: PROTOCOL_VERSION,
            server: SERVER_NAME.to_string(),
            capabilities: Self::CAPABILITIES.map(String::from).to_vec(),
            revision: ctx.store.revision()?,
        })
    }
}

/// Decodes `params`; omitted or `null` params decode as an empty object.
pub(crate) fn parse<T: DeserializeOwned>(params: &Value) -> Result<T, RpcError> {
    let value = match params {
        Value::Null => Value::Object(Map::new()),
        Value::Object(_) => params.clone(),
        _ => {
            return Err(RpcError::invalid_params(
                "params must be a JSON object or omitted",
            ));
        }
    };
    serde_json::from_value(value)
        .map_err(|error| RpcError::invalid_params(format!("invalid params: {error}")))
}

pub(crate) fn to_value<T: Serialize>(value: T) -> Result<Value, RpcError> {
    serde_json::to_value(value)
        .map_err(|error| RpcError::internal(format!("serialize result: {error}")))
}
