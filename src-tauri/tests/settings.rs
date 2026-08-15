use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use scribe_lib::settings::{
    Connection, Polling, Settings, ENV_AIRPORTS_FILE, ENV_AURORA_ADDR, ENV_ICAO,
};

fn temp_path() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    std::env::temp_dir().join(format!(
        "scribe-settings-{}-{}.json",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    move |name| map.get(name).cloned()
}

#[test]
fn the_defaults_match_the_specification() {
    let settings = Settings::default();
    assert_eq!(settings.aurora_addr().to_string(), "127.0.0.1:1130");
    assert_eq!(settings.ring_radius_nm, 20.0);
    assert_eq!(settings.airports_file, None);
    assert_eq!(settings.selected_icao, None);
    assert_eq!(settings.removal.parked_ground_speed_kt, 3);
    assert_eq!(settings.polling.emit_interval_ms, 100, "10 Hz emit ceiling");
    assert_eq!(settings.polling.flight_plan_max_attempts, 5);
}

#[test]
fn the_derived_configs_carry_the_settings_through() {
    let settings = Settings {
        ring_radius_nm: 25.0,
        connection: Connection {
            request_timeout_ms: 1_500,
            ..Connection::default()
        },
        ..Settings::default()
    };

    assert_eq!(settings.domain_config().ring_radius_nm, 25.0);
    assert_eq!(
        settings.client_config().request_timeout,
        std::time::Duration::from_millis(1_500)
    );
    assert_eq!(settings.client_config().addr, settings.aurora_addr());
}

#[test]
fn a_missing_file_yields_defaults_rather_than_an_error() {
    let path = temp_path();
    assert_eq!(
        Settings::load(&path).expect("defaults"),
        Settings::default()
    );
}

#[test]
fn settings_round_trip_through_the_file() {
    let path = temp_path();
    let settings = Settings {
        selected_icao: Some("LFLL".to_owned()),
        airports_file: Some(PathBuf::from("airports.txt")),
        ring_radius_nm: 30.0,
        polling: Polling {
            budget_requests_per_second: 120,
            ..Polling::default()
        },
        ..Settings::default()
    };

    settings.save(&path).expect("save");
    let loaded = Settings::load(&path).expect("load");
    assert_eq!(loaded, settings);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn saving_twice_overwrites_cleanly() {
    let path = temp_path();
    Settings::default().save(&path).expect("first save");
    let settings = Settings {
        ring_radius_nm: 15.0,
        ..Settings::default()
    };
    settings.save(&path).expect("second save");

    assert_eq!(Settings::load(&path).expect("load").ring_radius_nm, 15.0);
    assert!(
        !path.with_extension("tmp").exists(),
        "no temp file left behind"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_partial_file_falls_back_to_defaults_for_everything_else() {
    let path = temp_path();
    std::fs::write(&path, r#"{ "ringRadiusNm": 12.5 }"#).expect("write");

    let loaded = Settings::load(&path).expect("load");
    assert_eq!(loaded.ring_radius_nm, 12.5);
    assert_eq!(loaded.aurora_port, 1130);
    assert_eq!(loaded.polling, Settings::default().polling);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn broken_json_is_reported_rather_than_silently_ignored() {
    let path = temp_path();
    std::fs::write(&path, "{ not json").expect("write");
    assert!(Settings::load(&path).is_err());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn environment_overrides_win_over_the_file() {
    let mut settings = Settings::default();
    settings.apply_overrides(env(&[
        (ENV_AIRPORTS_FILE, "C:/atc/airports.txt"),
        (ENV_ICAO, "lfll"),
        (ENV_AURORA_ADDR, "127.0.0.1:2200"),
    ]));

    assert_eq!(
        settings.airports_file,
        Some(PathBuf::from("C:/atc/airports.txt"))
    );
    assert_eq!(settings.selected_icao.as_deref(), Some("LFLL"));
    assert_eq!(settings.aurora_port, 2200);
    assert_eq!(settings.aurora_host, IpAddr::V4(Ipv4Addr::LOCALHOST));
}

#[test]
fn an_unparseable_address_override_is_ignored() {
    let mut settings = Settings::default();
    settings.apply_overrides(env(&[(ENV_AURORA_ADDR, "not-an-address")]));
    assert_eq!(settings.aurora_addr().to_string(), "127.0.0.1:1130");
}

#[test]
fn no_overrides_leaves_the_settings_untouched() {
    let mut settings = Settings::default();
    settings.apply_overrides(env(&[]));
    assert_eq!(settings, Settings::default());
}
