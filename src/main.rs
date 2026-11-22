use chrono::{FixedOffset, Utc};
use clap::{Args, Parser, Subcommand};
use salah::{
    calculation::METHODS,
    config::{load, save},
    location, official, status,
};
use std::process::ExitCode;

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
    /// List the calculation methods
    Methods,
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
    match run(Cli::parse()) {
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
        Some(Command::Methods) => methods_command(),
    }
}

fn status_command(json: bool) -> Result<(), String> {
    let report = status::build(&load()?, Utc::now())?;
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
