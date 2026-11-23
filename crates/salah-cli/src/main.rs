use backlog::BacklogCommand;
use chrono::{DateTime, FixedOffset, Local as LocalTime, Utc};
use clap::{Args, Parser, Subcommand};
use epics::EpicCommand;
use local::Local;
use salah_core::{config::load, location};
use salah_domain::runtime::{Clock, FixedClock, SystemClock, UuidV4};
use salah_prayer::{calculation::METHODS, official};
use serde_json::{Map, Value};
use std::process::ExitCode;
use tracking::{
    DayArgs, ItemCommand, MetricCommand, PrayerCommand, ProjectCommand, SessionCommand, StatsArgs,
    TagCommand,
};

mod backlog;
mod daemon;
mod epics;
mod local;
mod render;
mod reports;
mod tracking;

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
    /// Plan items into prayer blocks
    Item {
        #[command(subcommand)]
        command: ItemCommand,
    },
    /// Time, log, and rate work
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    /// Manage projects
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// One-time tasks without a day yet, and scheduling them
    Backlog {
        #[command(subcommand)]
        command: BacklogCommand,
    },
    /// Manage a project's epics
    Epic {
        #[command(subcommand)]
        command: EpicCommand,
    },
    /// Manage tags
    Tag {
        #[command(subcommand)]
        command: TagCommand,
    },
    /// Manage metrics
    Metric {
        #[command(subcommand)]
        command: MetricCommand,
    },
    /// Show one day's blocks, items, and sessions
    Day(DayArgs),
    /// Mark prayers as prayed
    Prayer {
        #[command(subcommand)]
        command: PrayerCommand,
    },
    /// Show time and metric statistics
    Stats(StatsArgs),
    /// List finished sessions and their notes
    Sessions {
        #[command(subcommand)]
        command: reports::SessionsCommand,
    },
    /// Read JSON-RPC 2.0 requests from stdin and write responses to stdout
    Rpc(RpcArgs),
    /// Run the single-writer JSON-RPC daemon on the user Unix socket
    Serve(ServeArgs),
    /// List the calculation methods
    Methods,
    /// Open the Salah desktop app
    Open,
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
    let local = || Local::open(clock.as_ref(), &ids);
    match cli.command {
        None => status_command(local(), now, false),
        Some(Command::Status(args)) => status_command(local(), now, args.json),
        Some(Command::Config { command }) => config_command(&mut local()?, command),
        Some(Command::Location { command }) => location_command(command, now),
        Some(Command::Official { command }) => official_command(command, now),
        Some(Command::Item { command }) => tracking::item(&mut local()?, command),
        Some(Command::Session { command }) => tracking::session(&mut local()?, command),
        Some(Command::Project { command }) => tracking::project(&mut local()?, command),
        Some(Command::Backlog { command }) => backlog::backlog(&mut local()?, command),
        Some(Command::Epic { command }) => epics::epic(&mut local()?, command),
        Some(Command::Tag { command }) => tracking::tag(&mut local()?, command),
        Some(Command::Metric { command }) => tracking::metric(&mut local()?, command),
        Some(Command::Day(args)) => tracking::day(&mut local()?, args),
        Some(Command::Prayer { command }) => tracking::prayer(&mut local()?, command),
        Some(Command::Stats(mut args)) => match args.command.take() {
            Some(command) => reports::stats(&mut local()?, command),
            None => tracking::stats(&mut local()?, args),
        },
        Some(Command::Sessions { command }) => reports::sessions(&mut local()?, command),
        Some(Command::Rpc(args)) => match args.socket {
            Some(path) if path.is_empty() => daemon::bridge(None),
            Some(path) => daemon::bridge(Some(std::path::Path::new(&path))),
            None => local::serve_stdio(clock.as_ref(), &ids),
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

/// The bar label, or the whole `status.get` result with `--json`. Prayer
/// times still show when the store cannot be opened.
fn status_command(
    local: Result<Local<'_>, String>,
    now: DateTime<Utc>,
    json: bool,
) -> Result<(), String> {
    let status = match local {
        Ok(mut local) => local.call("status.get", Value::Null)?,
        Err(error) => {
            let mut status = salah_core::status::build(&load()?, now)?;
            status.planner_error = Some(error);
            serde_json::to_value(status).map_err(|e| e.to_string())?
        }
    };
    if json {
        println!("{status}");
    } else {
        println!("{}", status["label"].as_str().unwrap_or_default());
    }
    Ok(())
}

fn config_command(local: &mut Local<'_>, command: Option<ConfigCommand>) -> Result<(), String> {
    let config = match command {
        None | Some(ConfigCommand::Show) => local.call("config.get", Value::Null)?,
        Some(ConfigCommand::Set(args)) => local.call("config.set", args.params())?,
    };
    println!("{}", render::pretty(&config));
    Ok(())
}

impl ConfigSetArgs {
    /// `config.set` params without an op id: the human CLI applies directly.
    fn params(self) -> Value {
        let mut params = Map::new();
        let mut put = |key: &str, value: Option<Value>| {
            if let Some(value) = value {
                params.insert(key.into(), value);
            }
        };
        put("location", self.location.map(Value::from));
        put("latitude", self.latitude.map(Value::from));
        put("longitude", self.longitude.map(Value::from));
        put("timezone_offset", self.timezone_offset.map(Value::from));
        put("method", self.method.map(Value::from));
        put("asr", self.asr.map(Value::from));
        put("sleep_time", self.sleep_time.map(Value::from));
        put("auto_location", self.auto_location.map(Value::from));
        Value::Object(params)
    }
}

fn location_command(command: LocationCommand, now: DateTime<Utc>) -> Result<(), String> {
    match command {
        LocationCommand::Show => {
            println!(
                "{}",
                render::pretty(&serde_json::to_value(load()?).map_err(|e| e.to_string())?)
            );
            Ok(())
        }
        LocationCommand::Sync(args) => sync_location(args.if_needed, args.json, now),
    }
}

fn sync_location(if_needed: bool, json: bool, now: DateTime<Utc>) -> Result<(), String> {
    let local_timezone_offset = f64::from(LocalTime::now().offset().local_minus_utc()) / 3600.0;
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
