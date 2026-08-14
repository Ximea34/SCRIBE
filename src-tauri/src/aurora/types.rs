use crate::aurora::protocol::{AtcListRef, FlightPlanRef, TrafficListRef, TrafficPositionRef};

/// Owned `#FP` payload, produced once per callsign and then cached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlightPlan {
    pub callsign: Box<str>,
    pub dep: Box<str>,
    pub arr: Box<str>,
    pub alternate: Box<str>,
    pub eobt: Box<str>,
    pub aircraft: Box<str>,
    pub wake: Box<str>,
    pub rules: Box<str>,
    pub flight_type: Box<str>,
    pub equipment: Box<str>,
    pub cruise_level: Box<str>,
    pub cruise_speed: Box<str>,
    pub endurance: Box<str>,
    pub eet: Box<str>,
    pub route: Box<str>,
    pub remarks: Box<str>,
}

/// Owned `#TRPOS` payload. Field 19 is deliberately absent: its meaning is unknown (4.4).
#[derive(Debug, Clone, PartialEq)]
pub struct TrafficPosition {
    pub callsign: Box<str>,
    pub heading: Option<u16>,
    pub track: Option<u16>,
    pub altitude: Option<i32>,
    pub ground_speed: Option<u16>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub squawk_set: Box<str>,
    pub squawk_label: Box<str>,
    pub wp_label: Box<str>,
    pub alt_label: Box<str>,
    pub spd_label: Box<str>,
    pub assumed_by: Box<str>,
    pub next_station: Box<str>,
    pub on_ground: bool,
    pub is_selected: bool,
    pub was_selected: bool,
    pub gate: Box<str>,
    pub voice: Box<str>,
    pub vertical_speed: Option<i32>,
    pub assigned_gate: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtcPosition {
    pub station: Box<str>,
    pub frequency: Box<str>,
}

impl From<FlightPlanRef<'_>> for FlightPlan {
    fn from(f: FlightPlanRef<'_>) -> Self {
        Self {
            callsign: f.callsign().into(),
            dep: f.dep().into(),
            arr: f.arr().into(),
            alternate: f.alternate().into(),
            eobt: f.eobt().into(),
            aircraft: f.aircraft().into(),
            wake: f.wake().into(),
            rules: f.rules().into(),
            flight_type: f.flight_type().into(),
            equipment: f.equipment().into(),
            cruise_level: f.cruise_level().into(),
            cruise_speed: f.cruise_speed().into(),
            endurance: f.endurance().into(),
            eet: f.eet().into(),
            route: f.route().into(),
            remarks: f.remarks().into(),
        }
    }
}

impl From<TrafficPositionRef<'_>> for TrafficPosition {
    fn from(t: TrafficPositionRef<'_>) -> Self {
        Self {
            callsign: t.callsign().into(),
            heading: t.heading(),
            track: t.track(),
            altitude: t.altitude(),
            ground_speed: t.ground_speed(),
            lat: t.lat(),
            lon: t.lon(),
            squawk_set: t.squawk_set().into(),
            squawk_label: t.squawk_label().into(),
            wp_label: t.wp_label().into(),
            alt_label: t.alt_label().into(),
            spd_label: t.spd_label().into(),
            assumed_by: t.assumed_by().into(),
            next_station: t.next_station().into(),
            on_ground: t.on_ground(),
            is_selected: t.is_selected(),
            was_selected: t.was_selected(),
            gate: t.gate().into(),
            voice: t.voice().into(),
            vertical_speed: t.vertical_speed(),
            assigned_gate: t.assigned_gate().into(),
        }
    }
}

impl From<TrafficListRef<'_>> for Vec<Box<str>> {
    fn from(list: TrafficListRef<'_>) -> Self {
        list.iter().map(Box::<str>::from).collect()
    }
}

impl From<AtcListRef<'_>> for Vec<AtcPosition> {
    fn from(list: AtcListRef<'_>) -> Self {
        list.iter()
            .map(|(station, frequency)| AtcPosition {
                station: station.into(),
                frequency: frequency.into(),
            })
            .collect()
    }
}
