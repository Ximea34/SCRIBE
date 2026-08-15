use crate::domain::Flight;

/// Everything a paper strip needs, decoupled from the UI so the Zebra ZD410 path can generate
/// ZPL II from this alone. Nothing renders it yet; this is the declared seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripDocument {
    pub callsign: Box<str>,
    pub aircraft: Box<str>,
    pub wake: Box<str>,
    pub rules: Box<str>,
    pub flight_type: Box<str>,
    pub dep: Box<str>,
    pub arr: Box<str>,
    pub alternate: Box<str>,
    pub eobt: Box<str>,
    pub cruise_level: Box<str>,
    pub cruise_speed: Box<str>,
    pub route: Box<str>,
    pub remarks: Box<str>,
    pub squawk: Box<str>,
    pub stand: Box<str>,
    pub assumed_by: Box<str>,
}

impl From<&Flight> for StripDocument {
    fn from(flight: &Flight) -> Self {
        let position = flight.position.as_ref();
        Self {
            callsign: flight.callsign.clone(),
            aircraft: flight.plan.aircraft.clone(),
            wake: flight.plan.wake.clone(),
            rules: flight.plan.rules.clone(),
            flight_type: flight.plan.flight_type.clone(),
            dep: flight.plan.dep.clone(),
            arr: flight.plan.arr.clone(),
            alternate: flight.plan.alternate.clone(),
            eobt: flight.plan.eobt.clone(),
            cruise_level: flight.plan.cruise_level.clone(),
            cruise_speed: flight.plan.cruise_speed.clone(),
            route: flight.plan.route.clone(),
            remarks: flight.plan.remarks.clone(),
            squawk: position.map_or_else(Default::default, |p| p.squawk_set.clone()),
            stand: position.map_or_else(Default::default, |p| p.gate.clone()),
            assumed_by: position.map_or_else(Default::default, |p| p.assumed_by.clone()),
        }
    }
}
