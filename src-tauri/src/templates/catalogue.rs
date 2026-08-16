use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum FieldSource {
    FlightPlan,
    TrafficPosition,
    Derived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueEntry {
    /// Goes into the JSON and must never change; renaming one would orphan every saved template.
    pub key: String,
    pub label: String,
    pub source: FieldSource,
}

/// `#FP` fields in field-index order. `CALLSIGN` also exists in `#TRPOS`; it is exposed once.
const FLIGHT_PLAN: [&str; 16] = [
    "CALLSIGN",
    "DEPARTURE",
    "DESTINATION",
    "ALTERNATE",
    "EOBT",
    "AIRCRAFT",
    "WAKE",
    "RULES",
    "FLIGHT_TYPE",
    "EQUIPMENT",
    "CRUISE_LEVEL",
    "CRUISE_SPEED",
    "ENDURANCE",
    "EET",
    "ROUTE",
    "REMARKS",
];

/// `#TRPOS` fields in field-index order. 15/16 are screen state, 19 has no known meaning and 21
/// has never been verified against a real capture, so none of them appear here.
const TRAFFIC_POSITION: [&str; 17] = [
    "HEADING",
    "TRACK",
    "ALTITUDE",
    "GROUND_SPEED",
    "LATITUDE",
    "LONGITUDE",
    "SQUAWK_SET",
    "SQUAWK_LABEL",
    "WP_LABEL",
    "ALT_LABEL",
    "SPD_LABEL",
    "ASSUMED_BY",
    "NEXT_STATION",
    "ON_GROUND",
    "GATE",
    "VOICE",
    "VERTICAL_SPEED",
];

/// Conventional on real strips but not raw protocol fields; both resolve at print time.
const DERIVED: [&str; 2] = ["CONTROLLER_STATION", "PRINT_TIME"];

/// Protocol order, never alphabetical: controllers learn the positions and re-ordering them
/// between versions would be hostile.
pub fn catalogue() -> Vec<CatalogueEntry> {
    let groups = [
        (FLIGHT_PLAN.as_slice(), FieldSource::FlightPlan),
        (TRAFFIC_POSITION.as_slice(), FieldSource::TrafficPosition),
        (DERIVED.as_slice(), FieldSource::Derived),
    ];

    groups
        .into_iter()
        .flat_map(|(keys, source)| {
            keys.iter().map(move |key| CatalogueEntry {
                key: (*key).to_owned(),
                label: key.replace('_', " "),
                source,
            })
        })
        .collect()
}

pub fn contains(key: &str) -> bool {
    FLIGHT_PLAN.contains(&key) || TRAFFIC_POSITION.contains(&key) || DERIVED.contains(&key)
}
