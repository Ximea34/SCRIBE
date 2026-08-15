// Shared fixtures; each test binary uses a different subset, hence the blanket allow.
#![allow(dead_code)]

use scribe_lib::airports::{parse_airports, Airport};
use scribe_lib::aurora::protocol::{self, Response};
use scribe_lib::aurora::types::{FlightPlan, TrafficPosition};
use scribe_lib::domain::{DomainConfig, Store};
use scribe_lib::settings::Settings;

pub const LFLL_LINE: &str = "LFLL;LYON SAINT EXUPERY;45°43'32\"N;005°04'52\"E;821";
pub const LFLL_LAT: f64 = 45.725_555_555_555_55;
pub const LFLL_LON: f64 = 5.081_111_111_111_11;

pub fn lfll() -> Airport {
    let (registry, issues) = parse_airports(LFLL_LINE);
    assert!(issues.is_empty(), "fixture should be clean: {issues:?}");
    registry.get("LFLL").expect("LFLL is defined").clone()
}

pub fn config() -> DomainConfig {
    Settings::default().domain_config()
}

pub fn store() -> Store {
    Store::new(lfll(), config())
}

/// Built through the real parser, so the fixtures exercise the same field mapping as ingest.
pub fn plan(callsign: &str, dep: &str, arr: &str, eobt: &str, rules: &str) -> FlightPlan {
    let line = format!("#FP;{callsign};{dep};{arr};;{eobt};A320;M;{rules};S;;F330;N0450;;;;");
    match protocol::parse(&line).expect("flight plan fixture parses") {
        Response::FlightPlan(fp) => fp.into(),
        other => panic!("expected a flight plan, got {other:?}"),
    }
}

pub struct Where {
    pub lat: f64,
    pub lon: f64,
    pub altitude: i32,
    pub ground_speed: u16,
    pub on_ground: bool,
    pub gate: &'static str,
}

impl Where {
    pub fn airborne(lat: f64, lon: f64, altitude: i32) -> Self {
        Self {
            lat,
            lon,
            altitude,
            ground_speed: 250,
            on_ground: false,
            gate: "",
        }
    }

    pub fn grounded(lat: f64, lon: f64) -> Self {
        Self {
            lat,
            lon,
            altitude: 821,
            ground_speed: 0,
            on_ground: true,
            gate: "",
        }
    }

    pub fn speed(mut self, knots: u16) -> Self {
        self.ground_speed = knots;
        self
    }

    pub fn gate(mut self, gate: &'static str) -> Self {
        self.gate = gate;
        self
    }
}

pub fn position(callsign: &str, at: &Where) -> TrafficPosition {
    let ground = u8::from(at.on_ground);
    let Where {
        lat,
        lon,
        altitude,
        ground_speed,
        gate,
        ..
    } = at;
    let line = format!(
        "#TRPOS;{callsign};90;90;{altitude};{ground_speed};{lat:.9};{lon:.9};7000;7000;;;;;;{ground};0;0;{gate};V;;0;"
    );
    match protocol::parse(&line).expect("position fixture parses") {
        Response::TrafficPosition(tp) => tp.into(),
        other => panic!("expected a traffic position, got {other:?}"),
    }
}

/// Rough offset from the field, good enough to place traffic; exact distances are read back
/// from the store rather than assumed.
pub fn offset(bearing_degrees: f64, nautical_miles: f64) -> (f64, f64) {
    let bearing = bearing_degrees.to_radians();
    let lat = LFLL_LAT + nautical_miles * bearing.cos() / 60.0;
    let lon = LFLL_LON + nautical_miles * bearing.sin() / (60.0 * LFLL_LAT.to_radians().cos());
    (lat, lon)
}

/// Drives the full ingest path for one aircraft.
pub fn admit(store: &mut Store, callsign: &str, plan: FlightPlan, at: Option<&Where>, now: u64) {
    store.observe_radar([callsign], now);
    store.observe_flight_plan(callsign, plan, now);
    if let Some(at) = at {
        store.observe_position(callsign, position(callsign, at), now);
    }
}

pub fn column_of(store: &Store, callsign: &str) -> Option<scribe_lib::domain::Column> {
    store.flight(callsign).and_then(|f| f.state.column())
}
