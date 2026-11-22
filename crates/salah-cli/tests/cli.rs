use salah_core::config::Config;
use std::path::Path;
use std::process::{Command, Output};

fn run(config: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_salah"))
        .env("SALAH_CONFIG", config)
        .env("SALAH_DATA", config.with_extension("db"))
        .args(args)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn help_lists_subcommands() {
    let root = std::env::temp_dir().join(format!("salah-cli-main-help-{}", std::process::id()));
    let path = root.join("config.json");
    let output = run(&path, &["--help"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in [
        "status", "config", "location", "official", "item", "history", "export", "import",
        "methods",
    ] {
        assert!(
            stdout.contains(command),
            "missing {command} in help:\n{stdout}"
        );
    }
    assert!(!path.exists());
}

#[test]
fn config_set_help_succeeds_without_creating_a_config() {
    let root = std::env::temp_dir().join(format!("salah-cli-help-{}", std::process::id()));
    let path = root.join("config.json");
    let output = run(&path, &["config", "set", "--help"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
    assert!(!path.exists());
}

#[test]
fn invalid_updates_fail_without_overwriting_existing_config() {
    let root = std::env::temp_dir().join(format!("salah-cli-invalid-{}", std::process::id()));
    let path = root.join("config.json");
    salah_core::config::save_to(&Config::default(), &path).unwrap();
    let original = std::fs::read(&path).unwrap();
    for args in [
        vec!["config", "set", "--latitude", "91"],
        vec!["config", "set", "--longitude", "-181"],
        vec!["config", "set", "--timezone-offset", "15"],
        vec!["config", "set", "--method", "made-up"],
        vec!["config", "set", "--asr", "late"],
        vec!["config", "set", "--sleep-time", "24:00"],
        vec!["config", "set", "--sleep-time", "9:30"],
        vec!["config", "set", "--latitude", "NaN"],
        vec!["config", "set", "--unknown", "value"],
    ] {
        let output = run(&path, &args);
        assert!(!output.status.success(), "unexpected success for {args:?}");
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn commands_reject_ignored_or_incomplete_arguments() {
    let root = std::env::temp_dir().join(format!("salah-cli-args-{}", std::process::id()));
    let path = root.join("config.json");
    for args in [
        vec!["methods", "extra"],
        vec!["config", "show", "extra"],
        vec!["config", "set"],
        vec!["config", "set", "--latitude"],
        vec!["status", "--unknown"],
        vec!["status", "--json", "--json"],
        vec!["location"],
        vec!["location", "show", "extra"],
        vec!["location", "sync", "--unknown"],
    ] {
        let output = run(&path, &args);
        assert!(!output.status.success(), "unexpected success for {args:?}");
        assert!(!stderr(&output).trim().is_empty());
    }
}

#[test]
fn manual_location_disables_auto_detection_unless_explicitly_enabled() {
    let root = std::env::temp_dir().join(format!("salah-cli-auto-{}", std::process::id()));
    let path = root.join("config.json");
    let output = run(
        &path,
        &[
            "config",
            "set",
            "--location",
            "Samarkand",
            "--latitude",
            "39.6542",
            "--longitude",
            "66.9597",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(!salah_core::config::load_from(&path).unwrap().auto_location);

    let output = run(&path, &["config", "set", "--auto-location", "true"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(salah_core::config::load_from(&path).unwrap().auto_location);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn location_show_is_offline_and_reports_auto_state() {
    let root = std::env::temp_dir().join(format!("salah-cli-location-{}", std::process::id()));
    let path = root.join("config.json");
    let output = run(&path, &["location", "show"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["auto_location"], true);
    assert!(value["last_location_sync"].is_null());
}

#[test]
fn status_json_matches_the_plugin_contract() {
    let root = std::env::temp_dir().join(format!("salah-cli-json-{}", std::process::id()));
    let path = root.join("config.json");
    let output = run(&path, &["status", "--json"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value["location"].is_string());
    assert!(value["method"].is_string());
    assert!(value["next"]["name"].is_string());
    assert!(value["next"]["time"].is_string());
    assert!(value["next"]["remaining_seconds"].is_i64());
    assert!(value["next"]["kind"].is_string());
    assert!(value["current_block"]["start"]["id"].is_string());
    assert!(value["current_block"]["end"]["id"].is_string());
    assert!(value["current_block"]["sleeping"].is_boolean());
    assert!(value["prayers"].is_array());
    assert!(value["boundaries"].is_array());
}

#[test]
fn sleep_time_can_be_configured() {
    let root = std::env::temp_dir().join(format!("salah-cli-sleep-{}", std::process::id()));
    let path = root.join("config.json");
    let output = run(&path, &["config", "set", "--sleep-time", "22:45"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        salah_core::config::load_from(&path).unwrap().sleep_time,
        "22:45"
    );
    let _ = std::fs::remove_dir_all(root);
}
