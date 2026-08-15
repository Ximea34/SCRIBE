use crate::airports::Airport;

use super::flight::Flight;
use super::Millis;

/// What geometry and the flight plan alone can decide. Activation is deliberately absent:
/// the classifier cannot produce it, so a transit can never be classified into ACTIVÉS (5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoColumn {
    Awake,
    Arrival,
    Transit,
}

#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    pub airport: &'a Airport,
    pub ring_radius_nm: f64,
    pub max_position_age: Millis,
}

/// Pure column assignment. `None` means tracked but not displayed.
pub fn classify(flight: &Flight, now: Millis, context: Context<'_>) -> Option<AutoColumn> {
    let icao = &*context.airport.icao;

    // A departure from the controlled field stays a departure until the removal rules retire it,
    // which is what settles a flight that is both to and from the field.
    if flight.plan.dep.eq_ignore_ascii_case(icao) {
        return Some(AutoColumn::Awake);
    }

    let inside_ring = flight
        .fresh_distance_nm(now, context.max_position_age)
        .is_some_and(|nautical_miles| nautical_miles <= context.ring_radius_nm);
    if !inside_ring {
        return None;
    }

    if flight.plan.arr.eq_ignore_ascii_case(icao) {
        Some(AutoColumn::Arrival)
    } else {
        Some(AutoColumn::Transit)
    }
}
