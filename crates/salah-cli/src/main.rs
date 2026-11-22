use chrono::{DateTime, Duration, FixedOffset, Local, NaiveDate, Utc};
use clap::{Args, Parser, Subcommand};
use salah_core::{
    config::{load, save},
    export, location, planner, quick, status,
};
use salah_domain::{
    day::BoundaryId,
    item::{Measure, Repeat, State, Weekdays},
    runtime::{Clock, FixedClock, IdGenerator, SystemClock, UuidV4},
};
use salah_prayer::{calculation::METHODS, official, schedule};
use salah_protocol::{ConfigRepository, Context, Dispatcher};
use salah_store::{Store, default_path};
use std::{
    fs,
    io::{BufRead, Write},
    path::PathBuf,
    process::ExitCode,
};

mod daemon;

#[derive(Parser)]
#[command(
    name = "salah",
    about = "Prayer times with official online and calculated offline sources"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Show the next prayer or sleep boundary and the time remaining
    Status(StatusArgs),
    /// Show or change saved prayer settings
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommand>,
    },
    /// Show or update the saved location
    Location {
        #[command(subcommand)]
        command: LocationCommand,
    },
    /// Show or update the official prayer timetable
    Official {
        #[command(subcommand)]
        command: OfficialCommand,
    },
    /// Manage planned items
    Item {
        #[command(subcommand)]
        command: ItemCommand,
    },
    /// Show one day's prayers and items as a timeline
    Day(DayArgs),
    /// Mark prayers as prayed
    Prayer {
        #[command(subcommand)]
        command: PrayerCommand,
    },
    /// Show per-item and per-prayer statistics
    Stats(StatsArgs),
    /// Show planner history
    History(HistoryArgs),
    /// Export all planner data
    Export(ExportArgs),
    /// Import planner data
    Import(ImportArgs),
    /// Read JSON-RPC 2.0 requests from stdin and write responses to stdout
    Rpc(RpcArgs),
    /// Run the single-writer JSON-RPC daemon on the user Unix socket
    Serve(ServeArgs),
    /// List the calculation methods
    Methods,
    /// Open the Salah desktop app
    Open,
}

#[derive(Subcommand)]
enum ItemCommand {
    /// Add an item
    Add(ItemAddArgs),
    /// List today's items
    List(JsonArgs),
    /// Preview how quick-add text is understood, without saving
    Parse(ItemParseArgs),
    /// Move an item to a block and position
    Move(ItemMoveArgs),
    /// Mark an item done
    Done(ItemDateArgs),
    /// Undo an item's done state
    Undo(ItemDateArgs),
    /// Log minutes or a count
    Log(ItemLogArgs),
    /// Start an item's timer
    Start(ItemIdArgs),
    /// Stop the running timer, or only this item's timer
    Stop(ItemStopArgs),
    /// Edit an item
    Edit(ItemEditArgs),
    /// Remove an item
    Remove(ItemIdArgs),
}

#[derive(Args)]
struct ItemStopArgs {
    /// Item id or unique id prefix; without it, stop whichever timer runs
    id: Option<String>,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct JsonArgs {
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ItemIdArgs {
    /// Item id or unique id prefix
    id: String,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ItemDateArgs {
    /// Item id or unique id prefix
    id: String,
    /// Record on this day (YYYY-MM-DD) instead of now
    #[arg(long)]
    date: Option<String>,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ItemParseArgs {
    /// Quick-add text, e.g. "read 20m after isha daily"
    #[arg(required = true, num_args = 1..)]
    text: Vec<String>,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ItemMoveArgs {
    /// Item id or unique id prefix
    id: String,
    /// Target prayer block
    #[arg(long)]
    block: String,
    /// Position in the block, starting at 0
    #[arg(long)]
    index: usize,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Subcommand)]
enum PrayerCommand {
    /// Mark a prayer as prayed
    Mark(PrayerArgs),
    /// Remove a prayer's mark
    Unmark(PrayerArgs),
}

#[derive(Args)]
struct PrayerArgs {
    /// fajr, dhuhr, asr, maghrib, or isha
    prayer: String,
    /// Day (YYYY-MM-DD); defaults to the current day
    #[arg(long)]
    date: Option<String>,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct DayArgs {
    /// Day (YYYY-MM-DD); defaults to the current day
    #[arg(long)]
    date: Option<String>,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct StatsArgs {
    /// First day, formatted YYYY-MM-DD
    #[arg(long, requires = "to", conflicts_with = "days")]
    from: Option<String>,
    /// Last day, formatted YYYY-MM-DD
    #[arg(long, requires = "from")]
    to: Option<String>,
    /// Number of days ending today (default 30)
    #[arg(long)]
    days: Option<i64>,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ItemAddArgs {
    /// Words in the item title (or quick-add text with --quick)
    #[arg(required = true, num_args = 1..)]
    title: Vec<String>,
    /// Understand the title as quick-add text ("read 20m after isha daily")
    #[arg(long)]
    quick: bool,
    /// Prayer block
    #[arg(long)]
    block: Option<String>,
    /// Repeat: once, daily, weekdays, or weekly (with --days)
    #[arg(long, conflicts_with = "daily")]
    repeat: Option<String>,
    /// Weekdays for a weekly item, e.g. mon,wed,fri
    #[arg(long)]
    days: Option<String>,
    /// Same as --repeat daily
    #[arg(long)]
    daily: bool,
    /// Day to plan on (YYYY-MM-DD); defaults to today
    #[arg(long)]
    date: Option<String>,
    /// How progress is measured
    #[arg(long)]
    measure: Option<String>,
    /// Progress target
    #[arg(long)]
    target: Option<i64>,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ItemLogArgs {
    /// Item id or unique id prefix
    id: String,
    /// Minutes (or count, for count items) to add; negative subtracts
    #[arg(allow_negative_numbers = true)]
    value: i64,
    /// Adjust time spent in minutes, even for a count item
    #[arg(long)]
    minutes: bool,
    /// Record on this day (YYYY-MM-DD) instead of now
    #[arg(long)]
    date: Option<String>,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ItemEditArgs {
    /// Item id or unique id prefix
    id: String,
    /// Replacement title
    #[arg(long)]
    title: Option<String>,
    /// Replacement prayer block
    #[arg(long)]
    block: Option<String>,
    /// Replacement progress target
    #[arg(long, conflicts_with = "no_target")]
    target: Option<i64>,
    /// Remove the progress target
    #[arg(long)]
    no_target: bool,
    /// Replacement repeat: once, daily, weekdays, or weekly (with --days)
    #[arg(long)]
    repeat: Option<String>,
    /// Weekdays for a weekly item, e.g. mon,wed,fri
    #[arg(long)]
    days: Option<String>,
    /// Replacement measure: check, minutes, or count
    #[arg(long)]
    measure: Option<String>,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct HistoryArgs {
    /// First day, formatted YYYY-MM-DD
    #[arg(long)]
    from: Option<String>,
    /// Last day, formatted YYYY-MM-DD
    #[arg(long)]
    to: Option<String>,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ExportArgs {
    /// Write JSON to this file instead of stdout
    #[arg(long)]
    output: Option<PathBuf>,
    /// Replace an existing output file
    #[arg(long, requires = "output")]
    force: bool,
}

#[derive(Args)]
struct ImportArgs {
    /// Export file to import
    file: PathBuf,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct StatusArgs {
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct RpcArgs {
    /// Bridge stdin/stdout to a running `salah serve` instead of handling
    /// requests in-process; an optional path overrides the standard socket
    #[arg(long, value_name = "PATH", num_args = 0..=1, default_missing_value = "")]
    socket: Option<String>,
}

#[derive(Args)]
struct ServeArgs {
    /// Exit after this many minutes without connections when started by
    /// systemd socket activation (0 disables idle shutdown)
    #[arg(long, default_value_t = 5)]
    idle_minutes: u64,
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Show the saved configuration
    Show,
    /// Change one or more saved settings
    Set(ConfigSetArgs),
}

#[derive(Args)]
struct ConfigSetArgs {
    /// Set the location name
    #[arg(long)]
    location: Option<String>,
    /// Set the latitude in degrees
    #[arg(long, value_name = "DEGREES")]
    latitude: Option<f64>,
    /// Set the longitude in degrees
    #[arg(long, value_name = "DEGREES")]
    longitude: Option<f64>,
    /// Set the UTC offset in hours
    #[arg(long, value_name = "HOURS")]
    timezone_offset: Option<f64>,
    /// Set the calculation method
    #[arg(long, value_name = "ID")]
    method: Option<String>,
    /// Set the Asr calculation rule (standard or hanafi)
    #[arg(long)]
    asr: Option<String>,
    /// Set the sleep time
    #[arg(long, value_name = "HH:MM")]
    sleep_time: Option<String>,
    /// Enable or disable automatic location detection
    #[arg(long, value_name = "true or false")]
    auto_location: Option<bool>,
}

#[derive(Subcommand)]
enum LocationCommand {
    /// Show the saved location
    Show,
    /// Detect and save the current location
    #[command(
        long_about = "Location sync contacts geo.omarchy.org, then ipapi.co if needed; either provider sees this network's public IP address. The fallback's validated UTC offset is used when available, otherwise the system clock offset is used. The IP address is never stored. --if-needed skips the network when auto-location is disabled or a successful sync is less than six hours old."
    )]
    Sync(LocationSyncArgs),
}

#[derive(Args)]
struct LocationSyncArgs {
    /// Skip the network when auto-location is disabled or a recent sync is less than six hours old
    #[arg(long)]
    if_needed: bool,
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

#[derive(Subcommand)]
enum OfficialCommand {
    /// Sync the official Muslim Board timetable
    Sync(OfficialSyncArgs),
}

#[derive(Args)]
struct OfficialSyncArgs {
    /// Print machine-readable JSON
    #[arg(long)]
    json: bool,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            let _ = error.print();
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            let rendered = error.to_string();
            let message = rendered
                .lines()
                .next()
                .unwrap_or("invalid command")
                .strip_prefix("error: ")
                .unwrap_or(rendered.as_str());
            eprintln!("salah: {message}");
            return ExitCode::FAILURE;
        }
    };
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("salah: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let clock = clock_from_env()?;
    let ids = UuidV4;
    let now = clock.now();
    match cli.command {
        None => status_command(false, now),
        Some(Command::Status(args)) => status_command(args.json, now),
        Some(Command::Config { command }) => config_command(command),
        Some(Command::Location { command }) => location_command(command, now),
        Some(Command::Official { command }) => official_command(command, now),
        Some(Command::Item { command }) => item_command(command, now, &ids),
        Some(Command::Day(args)) => day_command(args, now),
        Some(Command::Prayer { command }) => prayer_command(command, now),
        Some(Command::Stats(args)) => stats_command(args, now),
        Some(Command::History(args)) => history_command(args, now),
        Some(Command::Export(args)) => export_command(args, now),
        Some(Command::Import(args)) => import_command(args, now),
        Some(Command::Rpc(args)) => match args.socket {
            Some(path) if path.is_empty() => daemon::bridge(None),
            Some(path) => daemon::bridge(Some(std::path::Path::new(&path))),
            None => rpc_command(clock.as_ref(), &ids),
        },
        Some(Command::Serve(args)) => daemon::serve(args.idle_minutes),
        Some(Command::Methods) => methods_command(),
        Some(Command::Open) => open_command(),
    }
}

/// Reads the test-only `SALAH_NOW` hook, falling back to the system clock.
fn clock_from_env() -> Result<Box<dyn Clock>, String> {
    match std::env::var("SALAH_NOW") {
        Ok(value) => {
            let now = DateTime::parse_from_rfc3339(&value)
                .map_err(|_| format!("invalid SALAH_NOW {value:?}; use RFC 3339"))?
                .with_timezone(&Utc);
            Ok(Box::new(FixedClock::new(now)))
        }
        Err(std::env::VarError::NotPresent) => Ok(Box::new(SystemClock)),
        Err(error) => Err(format!("invalid SALAH_NOW: {error}")),
    }
}

/// Adapter from the protocol's config port to the XDG JSON repository.
struct XdgConfig {
    path: PathBuf,
}

impl ConfigRepository for XdgConfig {
    fn load(&self) -> Result<salah_core::config::Config, salah_domain::error::Error> {
        salah_core::config::load_from(&self.path)
    }

    fn save(&self, config: &salah_core::config::Config) -> Result<(), salah_domain::error::Error> {
        salah_core::config::save_to(config, &self.path)
    }
}

/// `salah rpc`: one JSON-RPC object per input line, one per output line, until
/// stdin reaches EOF. The same dispatcher will back the S4 Unix socket daemon.
fn rpc_command(clock: &dyn Clock, ids: &dyn IdGenerator) -> Result<(), String> {
    let mut store = Store::open(&default_path())?;
    let config = XdgConfig {
        path: salah_core::config::path(),
    };
    let mut dispatcher = Dispatcher::new();
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut output = std::io::BufWriter::new(stdout.lock());
    let mut line = String::new();
    loop {
        line.clear();
        let read = input
            .read_line(&mut line)
            .map_err(|error| format!("read stdin: {error}"))?;
        if read == 0 {
            break;
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
            writeln!(output, "{encoded}").map_err(|error| format!("write stdout: {error}"))?;
            output
                .flush()
                .map_err(|error| format!("write stdout: {error}"))?;
        }
    }
    Ok(())
}

/// Starts `salah-app` from beside this binary when installed there, else from
/// PATH. The app runs detached so the terminal is free immediately.
fn open_command() -> Result<(), String> {
    let sibling = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("salah-app")))
        .filter(|path| path.is_file());
    let program = sibling.unwrap_or_else(|| std::path::PathBuf::from("salah-app"));
    std::process::Command::new(&program)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("cannot start the Salah app ({}): {e}", program.display()))
}

fn status_command(json: bool, now: DateTime<Utc>) -> Result<(), String> {
    let config = load()?;
    let mut report = status::build(&config, now)?;
    let planner =
        Store::open(&default_path()).and_then(|store| planner::today(&store, &config, now));
    report.attach_planner(planner);
    if json {
        println!(
            "{}",
            serde_json::to_string(&report).map_err(|e| e.to_string())?
        );
    } else {
        println!("{}", report.label);
    }
    Ok(())
}

fn open_planner() -> Result<(Store, salah_core::config::Config), String> {
    Ok((Store::open(&default_path())?, load()?))
}

fn parse_block(value: &str) -> Result<BoundaryId, String> {
    let block = BoundaryId::parse(value).ok_or_else(|| format!("unknown block {value:?}"))?;
    if block == BoundaryId::Sleep {
        return Err("sleep is not a planner block".into());
    }
    Ok(block)
}

fn parse_measure(value: Option<String>) -> Result<Option<Measure>, String> {
    value
        .map(|value| {
            serde_json::from_value(serde_json::Value::String(value.clone()))
                .map_err(|_| format!("unknown measure {value:?}; use check, minutes, or count"))
        })
        .transpose()
}

/// `--repeat` and `--days` into a repeat rule. `weekdays` is weekly Mon–Fri.
fn parse_repeat(
    repeat: Option<&str>,
    days: Option<&str>,
) -> Result<(Option<Repeat>, Option<Weekdays>), String> {
    let days = days.map(Weekdays::parse_list).transpose()?;
    match (repeat, days) {
        (None, None) => Ok((None, None)),
        (None, Some(_)) => Err("--days needs --repeat weekly".into()),
        (Some("once" | "none"), None) => Ok((Some(Repeat::None), None)),
        (Some("daily"), None) => Ok((Some(Repeat::Daily), None)),
        (Some("weekdays"), None) => Ok((Some(Repeat::Weekly), Some(Weekdays::WORKWEEK))),
        (Some("weekly"), Some(days)) => Ok((Some(Repeat::Weekly), Some(days))),
        (Some("weekly"), None) => Err("--repeat weekly needs --days, e.g. mon,wed".into()),
        (Some(other), None) => Err(format!(
            "unknown repeat {other:?}; use once, daily, weekdays, or weekly"
        )),
        (Some(_), Some(_)) => Err("--days needs --repeat weekly".into()),
    }
}

fn optional_date(value: Option<&str>) -> Result<Option<NaiveDate>, String> {
    value.map(parse_date).transpose()
}

fn json<T: serde::Serialize>(value: &T) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string(value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn short(id: &str) -> &str {
    &id[..id.len().min(8)]
}

fn item_command(
    command: ItemCommand,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
) -> Result<(), String> {
    let (mut store, config) = open_planner()?;
    match command {
        ItemCommand::List(args) => {
            let today = planner::today(&store, &config, now)?;
            if args.json {
                json(&today)
            } else {
                for item in &today.current {
                    let mark = if item.state == State::Done { "x" } else { " " };
                    println!("[{mark}] {}  {}", short(&item.id), item.title);
                }
                println!("{} open", today.open);
                Ok(())
            }
        }
        ItemCommand::Add(args) => {
            let text = args.title.join(" ");
            let mut request = if args.quick {
                quick::parse(&text)?.into_new_item()
            } else {
                planner::NewItem {
                    title: text,
                    ..planner::NewItem::default()
                }
            };
            // Explicit flags override what quick add understood.
            let repeat = args.repeat.as_deref().or(args.daily.then_some("daily"));
            let (repeat, weekdays) = parse_repeat(repeat, args.days.as_deref())?;
            if repeat.is_some() {
                request.repeat = repeat;
                request.weekdays = weekdays;
            }
            if let Some(block) = args.block.as_deref() {
                request.block = Some(parse_block(block)?);
            }
            if let Some(measure) = parse_measure(args.measure)? {
                request.measure = Some(measure);
            }
            if args.target.is_some() {
                request.target = args.target;
            }
            request.date = optional_date(args.date.as_deref())?;
            let item = planner::add(&mut store, &config, now, ids, request)?;
            if args.json {
                json(&item)
            } else {
                println!(
                    "Added {} \"{}\" to {}",
                    short(&item.id),
                    item.title,
                    item.block.name()
                );
                Ok(())
            }
        }
        ItemCommand::Parse(args) => {
            let draft = quick::parse(&args.text.join(" "))?;
            if args.json {
                json(&draft)
            } else {
                println!("{draft:?}");
                Ok(())
            }
        }
        ItemCommand::Move(args) => {
            let block = parse_block(&args.block)?;
            let item = planner::move_item(&mut store, &config, now, &args.id, block, args.index)?;
            if args.json {
                json(&item)
            } else {
                println!("Moved {} to {}", short(&item.id), item.block.name());
                Ok(())
            }
        }
        ItemCommand::Done(args) => {
            let date = optional_date(args.date.as_deref())?;
            let entry = planner::done(&mut store, &config, now, ids, &args.id, date)?;
            if args.json {
                json(&entry)
            } else {
                println!("Done {}", short(&entry.item_id));
                Ok(())
            }
        }
        ItemCommand::Undo(args) => {
            let date = optional_date(args.date.as_deref())?;
            let count = planner::undo(&mut store, &config, now, &args.id, date)?;
            if args.json {
                json(&serde_json::json!({"undone": count}))
            } else {
                println!("Undid {count} entry");
                Ok(())
            }
        }
        ItemCommand::Log(args) => {
            let date = optional_date(args.date.as_deref())?;
            let entry = if args.minutes {
                planner::adjust_time(&mut store, &config, now, ids, &args.id, args.value, date)?
            } else {
                planner::log(&mut store, &config, now, ids, &args.id, args.value, date)?
            };
            if args.json {
                json(&entry)
            } else {
                println!("Logged {} for {}", args.value, short(&entry.item_id));
                Ok(())
            }
        }
        ItemCommand::Start(args) => write_entry(
            &mut store,
            &config,
            now,
            ids,
            args,
            planner::start_timer,
            "Started",
        ),
        ItemCommand::Stop(args) => {
            let entries = match args.id.as_deref() {
                Some(id) => planner::stop_item_timer(&mut store, &config, now, id)?,
                None => planner::stop_timer(&mut store, &config, now)?,
            };
            if args.json {
                json(&entries)
            } else {
                println!("Stopped {} timer", entries.len());
                Ok(())
            }
        }
        ItemCommand::Edit(args) => {
            let target = if args.no_target {
                Some(None)
            } else {
                args.target.map(Some)
            };
            let item = planner::edit(&mut store, &config, now, &args.id, {
                let (repeat, weekdays) =
                    parse_repeat(args.repeat.as_deref(), args.days.as_deref())?;
                planner::ItemChanges {
                    title: args.title,
                    block: args.block.as_deref().map(parse_block).transpose()?,
                    target,
                    repeat,
                    weekdays,
                    measure: parse_measure(args.measure)?,
                }
            })?;
            if args.json {
                json(&item)
            } else {
                println!("Edited {} \"{}\"", short(&item.id), item.title);
                Ok(())
            }
        }
        ItemCommand::Remove(args) => {
            let item = planner::remove(&mut store, &config, now, &args.id)?;
            if args.json {
                json(&item)
            } else {
                println!("Removed {} \"{}\"", short(&item.id), item.title);
                Ok(())
            }
        }
    }
}

type EntryOperation = fn(
    &mut Store,
    &salah_core::config::Config,
    DateTime<Utc>,
    &dyn IdGenerator,
    &str,
) -> Result<salah_domain::item::Entry, salah_domain::error::Error>;

fn write_entry(
    store: &mut Store,
    config: &salah_core::config::Config,
    now: DateTime<Utc>,
    ids: &dyn IdGenerator,
    args: ItemIdArgs,
    operation: EntryOperation,
    verb: &str,
) -> Result<(), String> {
    let entry = operation(store, config, now, ids, &args.id)?;
    if args.json {
        json(&entry)
    } else {
        println!("{verb} {}", short(&entry.item_id));
        Ok(())
    }
}

fn day_command(args: DayArgs, now: DateTime<Utc>) -> Result<(), String> {
    let (store, config) = open_planner()?;
    let plan = planner::day_plan(&store, &config, now, optional_date(args.date.as_deref())?)?;
    if args.json {
        return json(&plan);
    }
    println!("{}", plan.day);
    let item_lines = |items: &[salah_domain::item::DayItem]| {
        for item in items {
            let tick = if item.state == State::Done { "x" } else { " " };
            println!("    [{tick}] {}  {}", short(&item.id), item.title);
        }
    };
    let night_now = plan
        .now
        .as_ref()
        .is_some_and(|n| n.block == Some(BoundaryId::Night));
    let now = if night_now { "  ← now" } else { "" };
    println!("  {:<8}{}{now}", "Night", plan.night_start);
    item_lines(&plan.blocks[0].items);
    for (prayer, block) in plan.prayers.iter().zip(&plan.blocks[1..]) {
        let mark = if prayer.marked { "●" } else { "○" };
        let now = if prayer.current { "  ← now" } else { "" };
        println!("{mark} {:<8}{}{now}", prayer.name, prayer.time);
        item_lines(&block.items);
    }
    println!("  Sleep   {}", plan.sleep.time);
    Ok(())
}

fn prayer_command(command: PrayerCommand, now: DateTime<Utc>) -> Result<(), String> {
    let (mut store, config) = open_planner()?;
    let (args, prayed) = match command {
        PrayerCommand::Mark(args) => (args, true),
        PrayerCommand::Unmark(args) => (args, false),
    };
    let prayer = parse_block(&args.prayer).map_err(|_| {
        format!(
            "unknown prayer {:?}; use fajr, dhuhr, asr, maghrib, or isha",
            args.prayer
        )
    })?;
    let date = optional_date(args.date.as_deref())?;
    let mark = if prayed {
        planner::mark(&mut store, &config, now, prayer, date)?
    } else {
        planner::unmark(&mut store, &config, now, prayer, date)?
    };
    if args.json {
        json(&mark)
    } else {
        let verb = if prayed { "Marked" } else { "Unmarked" };
        println!("{verb} {} on {}", prayer.name(), mark.day);
        Ok(())
    }
}

fn stats_command(args: StatsArgs, now: DateTime<Utc>) -> Result<(), String> {
    let (store, config) = open_planner()?;
    let (first, last) = match (args.from.as_deref(), args.to.as_deref()) {
        (Some(from), Some(to)) => (parse_date(from)?, parse_date(to)?),
        _ => {
            let days = args.days.unwrap_or(30);
            if !(1..=planner::MAX_HISTORY_DAYS).contains(&days) {
                return Err(format!(
                    "--days must be between 1 and {}",
                    planner::MAX_HISTORY_DAYS
                ));
            }
            let last = schedule::resolve(&config, now)?.day();
            (last - Duration::days(days - 1), last)
        }
    };
    let report = planner::stats(&store, &config, now, first, last)?;
    if args.json {
        return json(&report);
    }
    println!(
        "{} to {}: prayers {}/{}",
        report.from, report.to, report.marked_total, report.prayer_slots
    );
    for item in &report.items {
        println!(
            "  {}  {}/{} days",
            item.title, item.done_days, item.planned_days
        );
    }
    Ok(())
}

fn parse_date(value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| format!("invalid date {value:?}; use YYYY-MM-DD"))
}

fn history_command(args: HistoryArgs, now: DateTime<Utc>) -> Result<(), String> {
    let (store, config) = open_planner()?;
    let day = schedule::resolve(&config, now)?.day();
    let last = args
        .to
        .as_deref()
        .map(parse_date)
        .transpose()?
        .unwrap_or(day);
    let first = args
        .from
        .as_deref()
        .map(parse_date)
        .transpose()?
        .unwrap_or(last - Duration::days(6));
    let reports = planner::history(&store, &config, now, first, last)?;
    if args.json {
        json(&reports)
    } else {
        println!("{} days from {first} to {last}", reports.len());
        Ok(())
    }
}

fn export_command(args: ExportArgs, now: DateTime<Utc>) -> Result<(), String> {
    let (store, _) = open_planner()?;
    // S2 keeps the frozen v2 CLI shape; the core export format is v3 and
    // carries per-entity versions. S3 switches the CLI to the new envelope.
    let bytes =
        serde_json::to_vec(&export::export_legacy(&store, now)?).map_err(|e| e.to_string())?;
    if let Some(path) = args.output {
        atomic_write(&path, &bytes, args.force)?;
        println!("Exported to {}", path.display());
    } else {
        println!("{}", String::from_utf8(bytes).map_err(|e| e.to_string())?);
    }
    Ok(())
}

fn atomic_write(path: &std::path::Path, bytes: &[u8], force: bool) -> Result<(), String> {
    if path.exists() && !force {
        return Err(format!(
            "{} already exists; use --force to replace it",
            path.display()
        ));
    }
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid output file name")?;
    let temp = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    let result = (|| {
        fs::write(&temp, bytes).map_err(|e| format!("write {}: {e}", temp.display()))?;
        fs::rename(&temp, path).map_err(|e| format!("rename {}: {e}", path.display()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn import_command(args: ImportArgs, now: DateTime<Utc>) -> Result<(), String> {
    let metadata =
        fs::metadata(&args.file).map_err(|e| format!("read {}: {e}", args.file.display()))?;
    if metadata.len() > export::MAX_BYTES as u64 {
        return Err(format!(
            "export exceeds maximum size of {} bytes",
            export::MAX_BYTES
        ));
    }
    let bytes = fs::read(&args.file).map_err(|e| format!("read {}: {e}", args.file.display()))?;
    let data = export::parse(&bytes)?;
    let (mut store, config) = open_planner()?;
    let summary = store.write(|tx| {
        planner::close_expired_timers(tx, &config, now)?;
        export::import_tx(tx, &data)
    })?;
    if args.json {
        json(&summary)
    } else {
        println!(
            "Imported {} added, {} updated, {} unchanged",
            summary.added, summary.updated, summary.unchanged
        );
        Ok(())
    }
}

fn config_command(command: Option<ConfigCommand>) -> Result<(), String> {
    match command {
        None | Some(ConfigCommand::Show) => show_config(),
        Some(ConfigCommand::Set(args)) => set_config(args),
    }
}

fn show_config() -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(&load()?).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn set_config(args: ConfigSetArgs) -> Result<(), String> {
    let supplied = args.location.is_some()
        || args.latitude.is_some()
        || args.longitude.is_some()
        || args.timezone_offset.is_some()
        || args.method.is_some()
        || args.asr.is_some()
        || args.sleep_time.is_some()
        || args.auto_location.is_some();
    if !supplied {
        return Err("no settings supplied; run `salah config set --help`".into());
    }

    let manual_location = args.location.is_some()
        || args.latitude.is_some()
        || args.longitude.is_some()
        || args.timezone_offset.is_some();
    let explicit_auto_location = args.auto_location.is_some();
    let mut config = load()?;
    if let Some(value) = args.location {
        config.location = value;
    }
    if let Some(value) = args.latitude {
        config.latitude = value;
    }
    if let Some(value) = args.longitude {
        config.longitude = value;
    }
    if let Some(value) = args.timezone_offset {
        config.timezone_offset = value;
    }
    if let Some(value) = args.method {
        config.method = value;
    }
    if let Some(value) = args.asr {
        config.asr = value;
    }
    if let Some(value) = args.sleep_time {
        config.sleep_time = value;
    }
    if let Some(value) = args.auto_location {
        config.auto_location = value;
    }
    if manual_location && !explicit_auto_location {
        config.auto_location = false;
        config.last_location_sync = None;
    }
    config.validate()?;
    save(&config)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn location_command(command: LocationCommand, now: DateTime<Utc>) -> Result<(), String> {
    match command {
        LocationCommand::Show => show_config(),
        LocationCommand::Sync(args) => sync_location(args.if_needed, args.json, now),
    }
}

fn sync_location(if_needed: bool, json: bool, now: DateTime<Utc>) -> Result<(), String> {
    let local_timezone_offset = f64::from(Local::now().offset().local_minus_utc()) / 3600.0;
    let outcome = location::sync_saved(if_needed, now, local_timezone_offset)?;
    if json {
        println!(
            "{}",
            serde_json::to_string(&outcome).map_err(|e| e.to_string())?
        );
    } else {
        println!(
            "{} ({:.4}, {:.4}; system UTC{:+}; {})",
            outcome.config.location,
            outcome.config.latitude,
            outcome.config.longitude,
            outcome.config.timezone_offset,
            outcome.reason
        );
    }
    Ok(())
}

fn official_command(command: OfficialCommand, now: DateTime<Utc>) -> Result<(), String> {
    match command {
        OfficialCommand::Sync(args) => sync_official(args.json, now),
    }
}

fn sync_official(json: bool, now: DateTime<Utc>) -> Result<(), String> {
    let config = load()?;
    if config.method != "uzbekistan" || !config.location.to_ascii_lowercase().contains("tashkent") {
        return Err(
            "official Muslim Board sync currently supports Tashkent with the Uzbekistan method"
                .into(),
        );
    }
    let offset = FixedOffset::east_opt((config.timezone_offset * 3600.0).round() as i32)
        .ok_or("invalid timezone offset")?;
    let schedule = official::sync(now.with_timezone(&offset).date_naive(), now)?;
    if json {
        println!(
            "{}",
            serde_json::to_string(&schedule).map_err(|e| e.to_string())?
        );
    } else {
        println!("Muslim Board timetable updated for {}", schedule.date);
    }
    Ok(())
}

fn methods_command() -> Result<(), String> {
    for method in METHODS {
        println!("{}\t{}", method.id, method.name);
    }
    Ok(())
}
