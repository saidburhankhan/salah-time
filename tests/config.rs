use salah::config::{Config, load_from, save_to};
#[test]
fn config_round_trip() {
    let root = std::env::temp_dir().join(format!("salah-test-{}", std::process::id()));
    let path = root.join("config.json");
    let expected = Config::default();
    save_to(&expected, &path).unwrap();
    assert_eq!(load_from(&path).unwrap(), expected);
    let _ = std::fs::remove_dir_all(root);
}
#[test]
fn missing_config_uses_tashkent() {
    let path = std::env::temp_dir()
        .join(format!("missing-salah-{}", std::process::id()))
        .join("config.json");
    let c = load_from(&path).unwrap();
    assert_eq!(c.location, "Tashkent");
    assert_eq!(c.asr, "hanafi");
}

#[test]
fn legacy_config_without_auto_location_fields_still_loads() {
    let root = std::env::temp_dir().join(format!("salah-legacy-{}", std::process::id()));
    let path = root.join("config.json");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        &path,
        r#"{"location":"Bukhara","latitude":39.77,"longitude":64.42,"timezone_offset":5.0,"method":"mwl","asr":"hanafi"}"#,
    )
    .unwrap();
    let config = load_from(&path).unwrap();
    assert!(config.auto_location);
    assert_eq!(config.last_location_sync, None);
    let _ = std::fs::remove_dir_all(root);
}
