use chrono::{FixedOffset, Utc};
use salah::{
    calculation::METHODS,
    config::{load, save},
    location, official, status,
};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("salah: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None | Some("status") => status_command(args.get(1..).unwrap_or(&[])),
        Some("config") => config_command(&args[1..]),
        Some("location") => location_command(&args[1..]),
        Some("official") => official_command(&args[1..]),
        Some("methods") => {
            if args.len() != 1 {
                return Err("usage: salah methods".into());
            }
            for m in METHODS {
                println!("{}\t{}", m.id, m.name);
            }
            Ok(())
        }
        Some("help" | "--help" | "-h") => {
            help();
            Ok(())
        }
        Some(c) => Err(format!("unknown command: {c}; run `salah help`")),
    }
}

fn official_command(args: &[String]) -> Result<(), String> {
    match args {
        [command] if command == "sync" => sync_official(false),
        [command, flag] if command == "sync" && flag == "--json" => sync_official(true),
        _ => Err("usage: salah official sync [--json]".into()),
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

fn status_command(args: &[String]) -> Result<(), String> {
    let json = match args {
        [] => false,
        [flag] if flag == "--json" => true,
        _ => return Err("usage: salah status [--json]".into()),
    };
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

fn config_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None => {
            println!(
                "{}",
                serde_json::to_string_pretty(&load()?).map_err(|e| e.to_string())?
            );
            Ok(())
        }
        Some("show") if args.len() == 1 => {
            println!(
                "{}",
                serde_json::to_string_pretty(&load()?).map_err(|e| e.to_string())?
            );
            Ok(())
        }
        Some("show") => Err("usage: salah config show".into()),
        Some("set") => set_config(&args[1..]),
        Some("help" | "--help" | "-h") if args.len() == 1 => {
            config_help();
            Ok(())
        }
        Some(c) => Err(format!("unknown config command: {c}")),
    }
}

fn set_config(args: &[String]) -> Result<(), String> {
    if args.len() == 1 && matches!(args[0].as_str(), "help" | "--help" | "-h") {
        config_set_help();
        return Ok(());
    }
    if args.is_empty() {
        return Err("no settings supplied; run `salah config set --help`".into());
    }
    let mut config = load()?;
    let mut manual_location = false;
    let mut explicit_auto_location = false;
    let mut i = 0;
    while i < args.len() {
        let key = args[i].as_str();
        let value = args
            .get(i + 1)
            .ok_or_else(|| format!("missing value for {key}"))?;
        match key {
            "--location" => {
                config.location = value.clone();
                manual_location = true;
            }
            "--latitude" => {
                config.latitude = value.parse().map_err(|_| "invalid latitude")?;
                manual_location = true;
            }
            "--longitude" => {
                config.longitude = value.parse().map_err(|_| "invalid longitude")?;
                manual_location = true;
            }
            "--timezone-offset" => {
                config.timezone_offset = value.parse().map_err(|_| "invalid timezone offset")?;
                manual_location = true;
            }
            "--method" => config.method = value.clone(),
            "--asr" => config.asr = value.clone(),
            "--auto-location" => {
                config.auto_location = match value.as_str() {
                    "true" => true,
                    "false" => false,
                    _ => return Err("auto-location must be true or false".into()),
                };
                explicit_auto_location = true;
            }
            _ => return Err(format!("unknown option: {key}")),
        }
        i += 2;
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

fn location_command(args: &[String]) -> Result<(), String> {
    match args {
        [command] if command == "show" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&load()?).map_err(|e| e.to_string())?
            );
            Ok(())
        }
        [command] if command == "sync" => sync_location(false, false),
        [command, flag] if command == "sync" && flag == "--json" => sync_location(false, true),
        [command, flag] if command == "sync" && flag == "--if-needed" => sync_location(true, false),
        [command, first, second]
            if command == "sync"
                && ((first == "--if-needed" && second == "--json")
                    || (first == "--json" && second == "--if-needed")) =>
        {
            sync_location(true, true)
        }
        [flag] if matches!(flag.as_str(), "help" | "--help" | "-h") => {
            location_help();
            Ok(())
        }
        _ => Err("usage: salah location show | location sync [--if-needed] [--json]".into()),
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

fn help() {
    println!(
        "salah — prayer times with official online and calculated offline sources\n\n  salah status [--json]\n  salah location show\n  salah location sync [--if-needed] [--json]\n  salah official sync [--json]\n  salah methods\n  salah config show\n  salah config set [options]"
    );
}

fn config_help() {
    println!("usage: salah config show\n       salah config set [options]");
}

fn config_set_help() {
    println!(
        "usage: salah config set [--location NAME] [--latitude N] [--longitude N] [--timezone-offset HOURS] [--method ID] [--asr standard|hanafi] [--auto-location true|false]"
    );
}

fn location_help() {
    println!(
        "usage: salah location show\n       salah location sync [--if-needed] [--json]\n\nLocation sync contacts geo.omarchy.org, then ipapi.co if needed; either provider sees this network's public IP address. The fallback's validated UTC offset is used when available, otherwise the system clock offset is used. The IP address is never stored. --if-needed skips the network when auto-location is disabled or a successful sync is less than six hours old."
    );
}
