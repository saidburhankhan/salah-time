use chrono::{Duration, FixedOffset, NaiveDate, Utc};
use clap::{Args, Parser, Subcommand};
use salah::{
    calculation::METHODS,
    config::{load, save},
    day::BoundaryId,
    export,
    item::{Measure, Repeat, State},
    location, official, planner, schedule, status,
    store::{Store, default_path},
};
use std::{fs, path::PathBuf, process::ExitCode};

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
    /// Show planner history
    History(HistoryArgs),
    /// Export all planner data
    Export(ExportArgs),
    /// Import planner data
    Import(ImportArgs),
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
    /// Mark an item done
    Done(ItemIdArgs),
    /// Undo an item's done state
    Undo(ItemIdArgs),
    /// Log minutes or a count
    Log(ItemLogArgs),
    /// Start an item's timer
    Start(ItemIdArgs),
    /// Stop the running timer
    Stop(JsonArgs),
    /// Edit an item
    Edit(ItemEditArgs),
    /// Remove an item
    Remove(ItemIdArgs),
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
struct ItemAddArgs {
    /// Words in the item title
    #[arg(required = true, num_args = 1..)]
    title: Vec<String>,
    /// Prayer block
    #[arg(long)]
    block: Option<String>,
    /// Repeat the item daily
    #[arg(long)]
    daily: bool,
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
    /// Minutes or count to record
    value: i64,
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
    match cli.command {
        None => status_command(false),
        Some(Command::Status(args)) => status_command(args.json),
        Some(Command::Config { command }) => config_command(command),
        Some(Command::Location { command }) => location_command(command),
        Some(Command::Official { command }) => official_command(command),
        Some(Command::Item { command }) => item_command(command),
        Some(Command::History(args)) => history_command(args),
        Some(Command::Export(args)) => export_command(args),
        Some(Command::Import(args)) => import_command(args),
        Some(Command::Methods) => methods_command(),
        Some(Command::Open) => open_command(),
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

fn status_command(json: bool) -> Result<(), String> {
    let config = load()?;
    let now = Utc::now();
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

fn open_planner() -> Result<(Store, salah::config::Config, chrono::DateTime<Utc>), String> {
    Ok((Store::open(&default_path())?, load()?, Utc::now()))
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

fn item_command(command: ItemCommand) -> Result<(), String> {
    let (mut store, config, now) = open_planner()?;
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
            planner::close_expired_timers(&mut store, &config, now)?;
            let item = planner::add(
                &mut store,
                &config,
                now,
                planner::NewItem {
                    title: args.title.join(" "),
                    block: args.block.as_deref().map(parse_block).transpose()?,
                    repeat: args.daily.then_some(Repeat::Daily),
                    measure: parse_measure(args.measure)?,
                    target: args.target,
                },
            )?;
            if args.json {
                json(&item)
            } else {
                println!(
                    "Added {} \"{}\" to {:?}",
                    short(&item.id),
                    item.title,
                    item.block
                );
                Ok(())
            }
        }
        ItemCommand::Done(args) => {
            write_entry(&mut store, &config, now, args, planner::done, "Done")
        }
        ItemCommand::Undo(args) => {
            planner::close_expired_timers(&mut store, &config, now)?;
            let count = planner::undo(&mut store, &config, now, &args.id)?;
            if args.json {
                json(&serde_json::json!({"undone": count}))
            } else {
                println!("Undid {count} entry");
                Ok(())
            }
        }
        ItemCommand::Log(args) => {
            planner::close_expired_timers(&mut store, &config, now)?;
            let entry = planner::log(&mut store, &config, now, &args.id, args.value)?;
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
            args,
            planner::start_timer,
            "Started",
        ),
        ItemCommand::Stop(args) => {
            planner::close_expired_timers(&mut store, &config, now)?;
            let entries = planner::stop_timer(&mut store, &config, now)?;
            if args.json {
                json(&entries)
            } else {
                println!("Stopped {} timer", entries.len());
                Ok(())
            }
        }
        ItemCommand::Edit(args) => {
            planner::close_expired_timers(&mut store, &config, now)?;
            let target = if args.no_target {
                Some(None)
            } else {
                args.target.map(Some)
            };
            let item = planner::edit(
                &mut store,
                now,
                &args.id,
                planner::ItemChanges {
                    title: args.title,
                    block: args.block.as_deref().map(parse_block).transpose()?,
                    target,
                },
            )?;
            if args.json {
                json(&item)
            } else {
                println!("Edited {} \"{}\"", short(&item.id), item.title);
                Ok(())
            }
        }
        ItemCommand::Remove(args) => {
            planner::close_expired_timers(&mut store, &config, now)?;
            let item = planner::remove(&mut store, now, &args.id)?;
            if args.json {
                json(&item)
            } else {
                println!("Removed {} \"{}\"", short(&item.id), item.title);
                Ok(())
            }
        }
    }
}

fn write_entry(
    store: &mut Store,
    config: &salah::config::Config,
    now: chrono::DateTime<Utc>,
    args: ItemIdArgs,
    operation: fn(
        &mut Store,
        &salah::config::Config,
        chrono::DateTime<Utc>,
        &str,
    ) -> Result<salah::item::Entry, String>,
    verb: &str,
) -> Result<(), String> {
    planner::close_expired_timers(store, config, now)?;
    let entry = operation(store, config, now, &args.id)?;
    if args.json {
        json(&entry)
    } else {
        println!("{verb} {}", short(&entry.item_id));
        Ok(())
    }
}

fn parse_date(value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| format!("invalid date {value:?}; use YYYY-MM-DD"))
}

fn history_command(args: HistoryArgs) -> Result<(), String> {
    let (store, config, now) = open_planner()?;
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

fn export_command(args: ExportArgs) -> Result<(), String> {
    let (store, _, now) = open_planner()?;
    let bytes = serde_json::to_vec(&export::export(&store, now)?).map_err(|e| e.to_string())?;
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

fn import_command(args: ImportArgs) -> Result<(), String> {
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
    let (mut store, config, now) = open_planner()?;
    planner::close_expired_timers(&mut store, &config, now)?;
    let summary = export::import(&mut store, &data)?;
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

fn location_command(command: LocationCommand) -> Result<(), String> {
    match command {
        LocationCommand::Show => show_config(),
        LocationCommand::Sync(args) => sync_location(args.if_needed, args.json),
    }
}

fn sync_location(if_needed: bool, json: bool) -> Result<(), String> {
    let outcome = location::sync_saved(if_needed)?;
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

fn official_command(command: OfficialCommand) -> Result<(), String> {
    match command {
        OfficialCommand::Sync(args) => sync_official(args.json),
    }
}

fn sync_official(json: bool) -> Result<(), String> {
    let config = load()?;
    if config.method != "uzbekistan" || !config.location.to_ascii_lowercase().contains("tashkent") {
        return Err(
            "official Muslim Board sync currently supports Tashkent with the Uzbekistan method"
                .into(),
        );
    }
    let offset = FixedOffset::east_opt((config.timezone_offset * 3600.0).round() as i32)
        .ok_or("invalid timezone offset")?;
    let schedule = official::sync(Utc::now().with_timezone(&offset).date_naive())?;
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
