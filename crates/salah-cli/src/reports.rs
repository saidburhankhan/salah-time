//! Report subcommands: `salah stats table` and `salah sessions list`. Each
//! builds the params of one protocol method and prints its JSON result.

use crate::local::Local;
use clap::{Args, Subcommand};
use serde_json::{Map, Value, json};

/// The range and filter every report shares. `none` selects unassigned work.
#[derive(Args)]
pub struct ReportScope {
    #[arg(long, value_name = "YYYY-MM-DD", requires = "to")]
    from: Option<String>,
    #[arg(long, value_name = "YYYY-MM-DD", requires = "from")]
    to: Option<String>,
    /// Days ending today (default 30)
    #[arg(long, conflicts_with_all = ["from", "to"])]
    days: Option<i64>,
    /// Project id, or `none` for work without a project
    #[arg(long, value_name = "ID")]
    project: Option<String>,
    /// Tag id, or `none` for work without a tag
    #[arg(long, value_name = "ID")]
    tag: Option<String>,
    /// Item id or unique prefix, or `none` for work without a task
    #[arg(long, value_name = "ID")]
    item: Option<String>,
}

#[derive(Subcommand)]
pub enum StatsCommand {
    /// The general table: one row per week, day, project, tag or task
    Table(StatsTableArgs),
}

#[derive(Args)]
pub struct StatsTableArgs {
    #[command(flatten)]
    scope: ReportScope,
    /// week (default), day, project, tag or item
    #[arg(long)]
    group: Option<String>,
}

#[derive(Subcommand)]
pub enum SessionsCommand {
    /// Finished sessions newest first, with notes (the Notes page)
    List(SessionsListArgs),
}

#[derive(Args)]
pub struct SessionsListArgs {
    #[command(flatten)]
    scope: ReportScope,
    /// Include sessions without a note
    #[arg(long)]
    all: bool,
    /// Case-insensitive text in the note or any timestamped entry
    #[arg(long)]
    q: Option<String>,
    #[arg(long, value_name = "MINUTES")]
    min_minutes: Option<i64>,
    #[arg(long, value_name = "MINUTES")]
    max_minutes: Option<i64>,
    #[arg(long, value_name = "N")]
    min_focus: Option<f64>,
    #[arg(long, value_name = "N")]
    max_focus: Option<f64>,
    #[arg(long, value_name = "N")]
    min_quality: Option<f64>,
    #[arg(long, value_name = "N")]
    max_quality: Option<f64>,
    /// Lower bound of any metric, repeatable
    #[arg(long = "min", value_name = "METRIC=VALUE")]
    min: Vec<String>,
    /// Upper bound of any metric, repeatable
    #[arg(long = "max", value_name = "METRIC=VALUE")]
    max: Vec<String>,
    /// Rows per page (default 100, at most 500)
    #[arg(long)]
    limit: Option<i64>,
    /// The `next` cursor of the previous page
    #[arg(long)]
    before: Option<String>,
}

fn scope(local: &Local<'_>, scope: ReportScope) -> Result<Map<String, Value>, String> {
    let mut params = Map::new();
    let item = match scope.item {
        Some(id) if id == "none" => Some(id),
        Some(id) => Some(local.item_id(&id)?),
        None => None,
    };
    for (key, value) in [
        ("from", scope.from.map(Value::from)),
        ("to", scope.to.map(Value::from)),
        ("days", scope.days.map(Value::from)),
        ("project_id", scope.project.map(Value::from)),
        ("tag_id", scope.tag.map(Value::from)),
        ("item_id", item.map(Value::from)),
    ] {
        if let Some(value) = value {
            params.insert(key.into(), value);
        }
    }
    Ok(params)
}

fn print(result: &Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(result).map_err(|e| e.to_string())?
    );
    Ok(())
}

pub fn stats(local: &mut Local<'_>, command: StatsCommand) -> Result<(), String> {
    let StatsCommand::Table(args) = command;
    let mut params = scope(local, args.scope)?;
    if let Some(group) = args.group {
        params.insert("group".into(), group.into());
    }
    print(&local.call("stats.table", Value::Object(params))?)
}

pub fn sessions(local: &mut Local<'_>, command: SessionsCommand) -> Result<(), String> {
    let SessionsCommand::List(args) = command;
    let mut params = scope(local, args.scope)?;
    params.insert("with_note".into(), json!(!args.all));
    let mut put = |key: &str, value: Option<Value>| {
        if let Some(value) = value {
            params.insert(key.into(), value);
        }
    };
    put("q", args.q.map(Value::from));
    put("minutes_min", args.min_minutes.map(Value::from));
    put("minutes_max", args.max_minutes.map(Value::from));
    put("limit", args.limit.map(Value::from));
    put("before", args.before.map(Value::from));
    let mut bounds = Map::new();
    let mut bound = |metric: &str, side: &str, value: f64| {
        let entry = bounds.entry(metric).or_insert_with(|| json!({}));
        entry[side] = json!(value);
    };
    for (metric, side, value) in [
        ("focus", "min", args.min_focus),
        ("focus", "max", args.max_focus),
        ("quality", "min", args.min_quality),
        ("quality", "max", args.max_quality),
    ] {
        if let Some(value) = value {
            bound(metric, side, value);
        }
    }
    for (side, pairs) in [("min", &args.min), ("max", &args.max)] {
        for pair in pairs {
            let (metric, value) = pair
                .split_once('=')
                .ok_or_else(|| format!("{pair:?} is not METRIC=VALUE"))?;
            let value: f64 = value
                .trim()
                .parse()
                .map_err(|_| format!("{value:?} is not a number"))?;
            bound(metric.trim(), side, value);
        }
    }
    if !bounds.is_empty() {
        params.insert("values".into(), Value::Object(bounds));
    }
    print(&local.call("sessions.list", Value::Object(params))?)
}
