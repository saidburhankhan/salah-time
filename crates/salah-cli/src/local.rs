//! The in-process protocol client: every human subcommand and `salah rpc`
//! go through the same dispatcher as the socket daemon.

use salah_core::config::{self, Config};
use salah_domain::error::Error;
use salah_domain::runtime::{Clock, IdGenerator};
use salah_protocol::{ConfigRepository, Context, Dispatcher};
use salah_store::{Store, default_path};
use serde_json::Value;
use std::io::{BufRead, Write};
use std::path::PathBuf;

/// Adapter from the protocol's config port to the XDG JSON file.
pub struct XdgConfig {
    pub path: PathBuf,
}

impl XdgConfig {
    pub fn standard() -> Self {
        Self {
            path: config::path(),
        }
    }
}

impl ConfigRepository for XdgConfig {
    fn load(&self) -> Result<Config, Error> {
        config::load_from(&self.path)
    }

    fn save(&self, config: &Config) -> Result<(), Error> {
        config::save_to(config, &self.path)
    }
}

pub struct Local<'a> {
    pub store: Store,
    config: XdgConfig,
    clock: &'a dyn Clock,
    ids: &'a dyn IdGenerator,
    dispatcher: Dispatcher,
}

impl<'a> Local<'a> {
    pub fn open(clock: &'a dyn Clock, ids: &'a dyn IdGenerator) -> Result<Self, String> {
        let mut dispatcher = Dispatcher::new();
        dispatcher.accept_hello();
        Ok(Self {
            store: Store::open(&default_path())?,
            config: XdgConfig::standard(),
            clock,
            ids,
            dispatcher,
        })
    }

    /// Calls one method and returns its result, or the error message.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let mut context = Context {
            store: &mut self.store,
            config: &self.config,
            clock: self.clock,
            ids: self.ids,
        };
        self.dispatcher
            .call(method, &params, &mut context)
            .map_err(|error| error.message)
    }

    /// Calls a mutation under a fresh op id.
    pub fn mutate(&mut self, method: &str, mut params: Value) -> Result<Value, String> {
        params["op_id"] = Value::String(self.ids.new_id());
        self.call(method, params)
    }

    /// Reports also accept the literal `none` for sessions without a task.
    pub fn report_item_id(&self, id: &str) -> Result<String, String> {
        if id == salah_core::stats::NONE {
            Ok(id.to_owned())
        } else {
            Ok(salah_core::items::resolve_report_id(&self.store, id)?)
        }
    }

    /// Expands a unique item id prefix of at least four characters.
    pub fn item_id(&self, prefix: &str) -> Result<String, String> {
        Ok(salah_core::items::resolve_id(&self.store, prefix)?)
    }
}

/// `salah rpc`: one JSON-RPC request per stdin line, one response per stdout
/// line, until EOF. Unlike `Local`, the client must say `hello` first.
pub fn serve_stdio(clock: &dyn Clock, ids: &dyn IdGenerator) -> Result<(), String> {
    let mut store = Store::open(&default_path())?;
    let config = XdgConfig::standard();
    let mut dispatcher = Dispatcher::new();
    let mut input = std::io::stdin().lock();
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    let mut line = String::new();
    loop {
        line.clear();
        let read = input
            .read_line(&mut line)
            .map_err(|error| format!("read stdin: {error}"))?;
        if read == 0 {
            return Ok(());
        }
        let mut context = Context {
            store: &mut store,
            config: &config,
            clock,
            ids,
        };
        if let Some(response) = dispatcher.handle_line(&line, &mut context) {
            let encoded = response
                .into_line()
                .map_err(|error| format!("serialize response: {error}"))?;
            writeln!(output, "{encoded}")
                .and_then(|()| output.flush())
                .map_err(|error| format!("write stdout: {error}"))?;
        }
    }
}
