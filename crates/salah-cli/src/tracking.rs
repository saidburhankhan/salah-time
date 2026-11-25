//! Tracking subcommands. Each builds the params of one protocol method, calls
//! it in-process, and prints the result: the method's JSON with `--json`,
//! else a short human rendering.

use crate::local::Local;
use crate::render;
use clap::{Args, Subcommand};
use serde_json::{Map, Value, json};

#[derive(Args)]
pub struct Output {
    /// Print the protocol result as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum ItemCommand {
    /// Add an item
    Add(ItemAddArgs),
    /// List live items
    List {
        /// Also list routines past their last tracking day
        #[arg(long)]
        include_ended: bool,
        /// Include tasks belonging to archived projects
        #[arg(long)]
        include_paused: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Show how quick-add text is understood, without saving
    Parse(ItemParseArgs),
    /// Preview a draft's recurrence, without saving
    Preview(ItemPreviewArgs),
    /// Edit an item
    Edit(ItemEditArgs),
    /// Move an item to a block and position
    Move(ItemMoveArgs),
    /// Remove an item
    Remove(IdArgs),
}

#[derive(Subcommand)]
pub enum SessionCommand {
    /// Start a timer for an item or project
    Start(SessionStartArgs),
    /// Stop the running timer, optionally recording metrics
    Stop(SessionStopArgs),
    /// Record finished work, or tick an item without minutes
    Log(SessionLogArgs),
    /// Change a session
    Edit(SessionEditArgs),
    /// Add a timed note to a session
    Note(crate::session_notes::Add),
    /// Edit a timed note
    NoteEdit(crate::session_notes::Edit),
    /// Remove a timed note
    NoteRm(crate::session_notes::Target),
    /// Remove a session
    Remove(IdArgs),
}

#[derive(Subcommand)]
pub enum ProjectCommand {
    /// Count goals, routines and open tasks affected by archiving
    PausePreview(IdArgs),
    /// List projects
    List(ArchivedArgs),
    /// Add a project
    Add(ProjectAddArgs),
    /// Edit or archive a project
    Edit(ProjectEditArgs),
}

#[derive(Subcommand)]
pub enum TagCommand {
    /// List tags
    List(Output),
    /// Add a tag
    Add(TagAddArgs),
    /// Rename or recolour a tag
    Edit(TagEditArgs),
    /// Remove a tag from every project and delete it
    Remove(IdArgs),
}

#[derive(Subcommand)]
pub enum MetricCommand {
    /// List metrics
    List(ArchivedArgs),
    /// Add a metric
    Add(MetricAddArgs),
    /// Rename or archive a metric
    Edit(MetricEditArgs),
}

#[derive(Subcommand)]
pub enum PrayerCommand {
    /// Mark a prayer as prayed
    Mark(PrayerArgs),
    /// Remove a prayer's mark
    Unmark(PrayerArgs),
}

#[derive(Args)]
pub struct IdArgs {
    /// Entity id (an item id may be a unique prefix)
    id: String,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct ArchivedArgs {
    /// Only archived (true) or only active (false) rows
    #[arg(long)]
    archived: Option<bool>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct ItemAddArgs {
    /// Task tag id, repeatable. Pass --tag alone for an empty set.
    #[arg(long = "tag", value_name = "ID", num_args = 0..=1)]
    tags: Option<Vec<String>>,
    /// Title, or with --quick a line such as "read 20m after isha daily"
    #[arg(required = true, num_args = 1..)]
    title: Vec<String>,
    /// Parse the title as a quick-add line
    #[arg(long)]
    quick: bool,
    #[arg(long, value_name = "BLOCK")]
    block: Option<String>,
    /// none, daily, weekdays, or weekly
    #[arg(long)]
    repeat: Option<String>,
    /// Weekdays of a weekly item, e.g. mon,wed,fri
    #[arg(long, value_name = "DAYS")]
    days: Option<String>,
    #[arg(long, value_name = "ID")]
    project: Option<String>,
    /// Without --project the item joins the epic's project
    #[arg(long, value_name = "ID")]
    epic: Option<String>,
    #[arg(long, value_name = "MINUTES")]
    planned_minutes: Option<i64>,
    /// The one-time day, or the first day of a repeating item
    #[arg(long, value_name = "YYYY-MM-DD")]
    date: Option<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct ItemParseArgs {
    #[arg(required = true, num_args = 1..)]
    text: Vec<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct ItemPreviewArgs {
    #[arg(long)]
    block: String,
    #[arg(long)]
    repeat: String,
    #[arg(long, value_name = "DAYS")]
    days: Option<String>,
    #[arg(long, value_name = "MINUTES")]
    planned_minutes: Option<i64>,
    #[arg(long, value_name = "YYYY-MM-DD")]
    date: String,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct ItemEditArgs {
    /// One-time date when converting a routine (defaults to today)
    #[arg(long, value_name = "YYYY-MM-DD")]
    date: Option<String>,
    /// Last tracking day of a routine, inclusive
    #[arg(long, value_name = "YYYY-MM-DD", conflicts_with = "resume")]
    end_day: Option<String>,
    /// Clear the routine's end day
    #[arg(long)]
    resume: bool,
    /// Task tag id, repeatable. Pass --tag alone for an empty set.
    #[arg(long = "tag", value_name = "ID", num_args = 0..=1)]
    tags: Option<Vec<String>>,
    id: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    block: Option<String>,
    #[arg(long)]
    repeat: Option<String>,
    #[arg(long, value_name = "DAYS")]
    days: Option<String>,
    #[arg(long, value_name = "ID", conflicts_with = "no_project")]
    project: Option<String>,
    /// Detach the item from its project
    #[arg(long)]
    no_project: bool,
    /// An epic of the item's (new) project
    #[arg(long, value_name = "ID", conflicts_with_all = ["no_epic", "no_project"])]
    epic: Option<String>,
    /// Detach the item from its epic
    #[arg(long)]
    no_epic: bool,
    #[arg(long, value_name = "MINUTES", conflicts_with = "no_planned_minutes")]
    planned_minutes: Option<i64>,
    /// Clear the planned minutes
    #[arg(long)]
    no_planned_minutes: bool,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct ItemMoveArgs {
    id: String,
    #[arg(long)]
    block: String,
    /// Zero-based position in the block
    #[arg(long, default_value_t = 0)]
    index: usize,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct SessionStartArgs {
    #[arg(long, value_name = "ID")]
    item: Option<String>,
    #[arg(long, value_name = "ID")]
    project: Option<String>,
    /// Work on an epic without a task; its project is recorded too
    #[arg(long, value_name = "ID")]
    epic: Option<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct SessionStopArgs {
    /// The running session; default: whichever runs
    #[arg(long)]
    id: Option<String>,
    /// Override worked minutes, up to the elapsed time
    #[arg(long)]
    minutes: Option<i64>,
    /// Also tick the item on the timer's planning day
    #[arg(long)]
    done: bool,
    /// A metric value, e.g. --value focus=4 (repeatable)
    #[arg(long = "value", value_name = "METRIC=VALUE")]
    values: Vec<String>,
    #[arg(long)]
    note: Option<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct SessionLogArgs {
    #[arg(long, value_name = "ID")]
    item: Option<String>,
    #[arg(long, value_name = "ID")]
    project: Option<String>,
    /// Work on an epic without a task; its project is recorded too
    #[arg(long, value_name = "ID")]
    epic: Option<String>,
    #[arg(long, value_name = "YYYY-MM-DD")]
    date: Option<String>,
    #[arg(long)]
    block: Option<String>,
    /// Without minutes the session is a tick
    #[arg(long)]
    minutes: Option<i64>,
    #[arg(long = "value", value_name = "METRIC=VALUE")]
    values: Vec<String>,
    #[arg(long)]
    note: Option<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct SessionEditArgs {
    id: String,
    #[arg(long)]
    minutes: Option<i64>,
    /// Replaces every value when given
    #[arg(long = "value", value_name = "METRIC=VALUE")]
    values: Vec<String>,
    #[arg(long)]
    note: Option<String>,
    #[arg(long)]
    block: Option<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct ProjectAddArgs {
    name: String,
    /// A #rrggbb colour
    #[arg(long)]
    color: Option<String>,
    #[arg(long)]
    note: Option<String>,
    #[arg(long = "tag", value_name = "ID")]
    tags: Vec<String>,
    /// A metric to ask for, `ID` or `ID:optional`; all answers are optional
    #[arg(long = "metric", value_name = "ID[:optional]")]
    metrics: Vec<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct ProjectEditArgs {
    id: String,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    color: Option<String>,
    #[arg(long)]
    note: Option<String>,
    /// Replaces the project's tags when given
    #[arg(long = "tag", value_name = "ID")]
    tags: Vec<String>,
    /// Replaces the project's metrics when given
    #[arg(long = "metric", value_name = "ID[:optional]")]
    metrics: Vec<String>,
    #[arg(long)]
    archived: Option<bool>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct TagAddArgs {
    name: String,
    #[arg(long)]
    color: String,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct TagEditArgs {
    id: String,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    color: Option<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct MetricAddArgs {
    name: String,
    /// count, scale, or check
    #[arg(long)]
    kind: String,
    #[arg(long, default_value = "")]
    unit: String,
    #[arg(long)]
    scale_max: Option<i64>,
    /// sum or avg
    #[arg(long)]
    rollup: Option<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct MetricEditArgs {
    id: String,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    unit: Option<String>,
    #[arg(long)]
    archived: Option<bool>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct PrayerArgs {
    /// fajr, dhuhr, asr, maghrib, or isha
    prayer: String,
    #[arg(long, value_name = "YYYY-MM-DD")]
    date: Option<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
pub struct DayArgs {
    #[arg(long, value_name = "YYYY-MM-DD")]
    date: Option<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct StatsArgs {
    #[command(subcommand)]
    pub command: Option<crate::reports::StatsCommand>,
    #[arg(long, value_name = "YYYY-MM-DD", requires = "to")]
    from: Option<String>,
    #[arg(long, value_name = "YYYY-MM-DD", requires = "from")]
    to: Option<String>,
    /// Days ending today (default 30)
    #[arg(long, conflicts_with_all = ["from", "to"])]
    days: Option<i64>,
    #[arg(long, value_name = "ID")]
    project: Option<String>,
    #[arg(long, value_name = "ID")]
    tag: Option<String>,
    /// Epic id, or `none` for work outside any epic
    #[arg(long, value_name = "ID")]
    epic: Option<String>,
    #[arg(long, value_name = "ID")]
    item: Option<String>,
    #[command(flatten)]
    measures: Box<crate::reports::MeasureArgs>,
    #[command(flatten)]
    output: Output,
}

/// JSON params built from optional flags; absent flags are left out.
#[derive(Default)]
pub(crate) struct Params(Map<String, Value>);

impl Params {
    pub(crate) fn set(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.0.insert(key.into(), value.into());
        self
    }

    pub(crate) fn opt(self, key: &str, value: Option<impl Into<Value>>) -> Self {
        match value {
            Some(value) => self.set(key, value),
            None => self,
        }
    }

    pub(crate) fn done(self) -> Value {
        Value::Object(self.0)
    }
}

/// `--repeat weekdays` is shorthand for weekly on Monday to Friday and
/// `once` for `none`; `--days` lists the weekdays of a weekly item.
fn repeat(params: Params, repeat: Option<String>, days: Option<String>) -> Params {
    let (repeat, days) = match repeat.as_deref() {
        Some("weekdays") => (
            Some("weekly".to_owned()),
            Some("mon,tue,wed,thu,fri".to_owned()),
        ),
        Some("once") => (Some("none".to_owned()), days),
        _ => (repeat, days),
    };
    let weekdays = days.map(|list| {
        list.split(',')
            .map(|day| day.trim().to_owned())
            .collect::<Vec<_>>()
    });
    params.opt("repeat", repeat).opt("weekdays", weekdays)
}

fn values(pairs: &[String]) -> Result<Value, String> {
    let mut values = Map::new();
    for pair in pairs {
        let (metric, value) = pair
            .split_once('=')
            .ok_or_else(|| format!("{pair:?} is not METRIC=VALUE"))?;
        let value: f64 = value
            .trim()
            .parse()
            .map_err(|_| format!("{value:?} is not a number"))?;
        values.insert(metric.trim().into(), json!(value));
    }
    Ok(Value::Object(values))
}

fn metric_choices(choices: &[String]) -> Value {
    choices
        .iter()
        .map(|choice| match choice.split_once(':') {
            Some((id, "optional")) => json!({"metric_id": id, "required": false}),
            _ => json!({"metric_id": choice, "required": true}),
        })
        .collect()
}

pub(crate) fn print(
    output: &Output,
    result: &Value,
    human: impl Fn(&Value) -> String,
) -> Result<(), String> {
    if output.json {
        println!("{result}");
    } else {
        println!("{}", human(result));
    }
    Ok(())
}

pub fn item(local: &mut Local<'_>, command: ItemCommand) -> Result<(), String> {
    match command {
        ItemCommand::Add(a) => {
            let text = a.title.join(" ");
            let params = Params::default()
                .opt("tag_ids", a.tags)
                .set(if a.quick { "quick_text" } else { "title" }, text)
                .opt("block", a.block)
                .opt("project_id", a.project)
                .opt("epic_id", a.epic)
                .opt("planned_minutes", a.planned_minutes)
                .opt("date", a.date);
            let result = local.mutate("item.add", repeat(params, a.repeat, a.days).done())?;
            print(&a.output, &result, render::mutation)
        }
        ItemCommand::List {
            include_ended,
            include_paused,
            output,
        } => {
            let result = local.call(
                "items.list",
                json!({"include_ended":include_ended,"include_paused":include_paused}),
            )?;
            print(&output, &result, render::items)
        }
        ItemCommand::Parse(a) => {
            let result = local.call("item.parse", json!({"text": a.text.join(" ")}))?;
            print(&a.output, &result, render::pretty)
        }
        ItemCommand::Preview(a) => {
            let params = Params::default()
                .set("block", a.block)
                .set("date", a.date)
                .opt("planned_minutes", a.planned_minutes);
            let result = local.call(
                "item.preview",
                repeat(params, Some(a.repeat), a.days).done(),
            )?;
            print(&a.output, &result, render::pretty)
        }
        ItemCommand::Edit(a) => {
            let mut params = Params::default()
                .set("id", local.item_id(&a.id)?)
                .opt("tag_ids", a.tags)
                .opt("title", a.title)
                .opt("date", a.date)
                .opt("end_day", a.end_day)
                .opt("block", a.block)
                .opt("project_id", a.project)
                .opt("planned_minutes", a.planned_minutes);
            if a.resume {
                params = params.set("end_day", Value::Null);
            }
            if a.no_project {
                params = params.set("project_id", Value::Null);
            }
            params = params.opt("epic_id", a.epic);
            if a.no_epic {
                params = params.set("epic_id", Value::Null);
            }
            if a.no_planned_minutes {
                params = params.set("planned_minutes", Value::Null);
            }
            let result = local.mutate("item.edit", repeat(params, a.repeat, a.days).done())?;
            print(&a.output, &result, render::mutation)
        }
        ItemCommand::Move(a) => {
            let params = json!({"id": local.item_id(&a.id)?, "block": a.block, "index": a.index});
            let result = local.mutate("item.move", params)?;
            print(&a.output, &result, render::mutation)
        }
        ItemCommand::Remove(a) => {
            let result = local.mutate("item.remove", json!({"id": local.item_id(&a.id)?}))?;
            print(&a.output, &result, render::mutation)
        }
    }
}

pub fn session(local: &mut Local<'_>, command: SessionCommand) -> Result<(), String> {
    let (method, params, output) = match command {
        SessionCommand::Start(a) => {
            let item = a.item.map(|id| local.item_id(&id)).transpose()?;
            let params = Params::default()
                .opt("item_id", item)
                .opt("project_id", a.project)
                .opt("epic_id", a.epic);
            ("session.start", params.done(), a.output)
        }
        SessionCommand::Stop(a) => {
            let params = Params::default()
                .opt("id", a.id)
                .opt("minutes", a.minutes)
                .set("done", a.done)
                .set("values", values(&a.values)?)
                .opt("note", a.note);
            ("session.stop", params.done(), a.output)
        }
        SessionCommand::Log(a) => {
            let item = a.item.map(|id| local.item_id(&id)).transpose()?;
            let params = Params::default()
                .opt("item_id", item)
                .opt("project_id", a.project)
                .opt("epic_id", a.epic)
                .opt("date", a.date)
                .opt("block", a.block)
                .opt("minutes", a.minutes)
                .set("values", values(&a.values)?)
                .opt("note", a.note);
            ("session.log", params.done(), a.output)
        }
        SessionCommand::Edit(a) => {
            let given = (!a.values.is_empty())
                .then(|| values(&a.values))
                .transpose()?;
            let params = Params::default()
                .set("id", a.id)
                .opt("minutes", a.minutes)
                .opt("values", given)
                .opt("note", a.note)
                .opt("block", a.block);
            ("session.edit", params.done(), a.output)
        }
        SessionCommand::Note(a) => return crate::session_notes::add(local, a),
        SessionCommand::NoteEdit(a) => return crate::session_notes::edit(local, a),
        SessionCommand::NoteRm(a) => return crate::session_notes::remove(local, a),
        SessionCommand::Remove(a) => ("session.remove", json!({"id": a.id}), a.output),
    };
    let result = local.mutate(method, params)?;
    print(&output, &result, render::mutation)
}

pub fn project(local: &mut Local<'_>, command: ProjectCommand) -> Result<(), String> {
    match command {
        ProjectCommand::PausePreview(a) => {
            let result = local.call("project.pause_preview", json!({"id":a.id}))?;
            print(&a.output, &result, render::pretty)
        }
        ProjectCommand::List(a) => {
            let result = local.call(
                "projects.list",
                Params::default().opt("archived", a.archived).done(),
            )?;
            print(&a.output, &result, render::projects)
        }
        ProjectCommand::Add(a) => {
            let params = Params::default()
                .set("name", a.name)
                .opt("color", a.color)
                .opt("note", a.note)
                .set("tag_ids", a.tags)
                .opt(
                    "metrics",
                    (!a.metrics.is_empty()).then(|| metric_choices(&a.metrics)),
                );
            let result = local.mutate("project.add", params.done())?;
            print(&a.output, &result, render::mutation)
        }
        ProjectCommand::Edit(a) => {
            let params = Params::default()
                .set("id", a.id)
                .opt("name", a.name)
                .opt("color", a.color)
                .opt("note", a.note)
                .opt("tag_ids", (!a.tags.is_empty()).then_some(a.tags))
                .opt(
                    "metrics",
                    (!a.metrics.is_empty()).then(|| metric_choices(&a.metrics)),
                )
                .opt("archived", a.archived);
            let result = local.mutate("project.edit", params.done())?;
            print(&a.output, &result, render::mutation)
        }
    }
}

pub fn tag(local: &mut Local<'_>, command: TagCommand) -> Result<(), String> {
    let (method, params, output) = match command {
        TagCommand::List(output) => {
            let result = local.call("tags.list", Value::Null)?;
            return print(&output, &result, render::named_rows);
        }
        TagCommand::Add(a) => (
            "tag.add",
            json!({"name": a.name, "color": a.color}),
            a.output,
        ),
        TagCommand::Edit(a) => {
            let params = Params::default()
                .set("id", a.id)
                .opt("name", a.name)
                .opt("color", a.color);
            ("tag.edit", params.done(), a.output)
        }
        TagCommand::Remove(a) => ("tag.remove", json!({"id": a.id}), a.output),
    };
    let result = local.mutate(method, params)?;
    print(&output, &result, render::mutation)
}

pub fn metric(local: &mut Local<'_>, command: MetricCommand) -> Result<(), String> {
    let (method, params, output) = match command {
        MetricCommand::List(a) => {
            let result = local.call(
                "metrics.list",
                Params::default().opt("archived", a.archived).done(),
            )?;
            return print(&a.output, &result, render::named_rows);
        }
        MetricCommand::Add(a) => {
            let params = Params::default()
                .set("name", a.name)
                .set("kind", a.kind)
                .set("unit", a.unit)
                .opt("scale_max", a.scale_max)
                .opt("rollup", a.rollup);
            ("metric.add", params.done(), a.output)
        }
        MetricCommand::Edit(a) => {
            let params = Params::default()
                .set("id", a.id)
                .opt("name", a.name)
                .opt("unit", a.unit)
                .opt("archived", a.archived);
            ("metric.edit", params.done(), a.output)
        }
    };
    let result = local.mutate(method, params)?;
    print(&output, &result, render::mutation)
}

pub fn prayer(local: &mut Local<'_>, command: PrayerCommand) -> Result<(), String> {
    let (method, args) = match command {
        PrayerCommand::Mark(args) => ("prayer.mark", args),
        PrayerCommand::Unmark(args) => ("prayer.unmark", args),
    };
    let params = Params::default()
        .set("prayer", args.prayer.to_ascii_lowercase())
        .opt("date", args.date);
    let result = local.mutate(method, params.done())?;
    print(&args.output, &result, render::mutation)
}

pub fn day(local: &mut Local<'_>, args: DayArgs) -> Result<(), String> {
    let result = local.call("day.get", Params::default().opt("date", args.date).done())?;
    print(&args.output, &result, render::day)
}

pub fn stats(local: &mut Local<'_>, args: StatsArgs) -> Result<(), String> {
    let item = args.item.map(|id| local.report_item_id(&id)).transpose()?;
    let params = Params::default()
        .opt("from", args.from)
        .opt("to", args.to)
        .opt("days", args.days)
        .opt("project_id", args.project)
        .opt("tag_id", args.tag)
        .opt("epic_id", args.epic)
        .opt("item_id", item);
    let mut params = params.done();
    if let Value::Object(map) = &mut params {
        crate::reports::measures(map, &args.measures)?;
    }
    let result = local.call("stats.get", params)?;
    print(&args.output, &result, render::stats)
}
