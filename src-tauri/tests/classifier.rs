mod common;

use common::{lfll, offset, plan, position, Where};
use scribe_lib::airports::Airport;
use scribe_lib::domain::classifier::{classify, AutoColumn, Context};
use scribe_lib::domain::Flight;

const MAX_POSITION_AGE: u64 = 15_000;
const RING: f64 = 20.0;

fn context(airport: &Airport, ring_radius_nm: f64) -> Context<'_> {
    Context {
        airport,
        ring_radius_nm,
        max_position_age: MAX_POSITION_AGE,
    }
}

fn flight(callsign: &str, dep: &str, arr: &str, at: Option<Where>, airport: &Airport) -> Flight {
    let mut flight = Flight::new(callsign, plan(callsign, dep, arr, "1215", "I"), 0);
    if let Some(at) = at {
        flight.set_position(position(callsign, &at), airport.centre(), 0);
    }
    flight
}

#[test]
fn a_departure_from_the_field_is_awake_even_without_a_position() {
    let airport = lfll();
    let flight = flight("AFR1234", "LFLL", "LFPG", None, &airport);
    assert_eq!(
        classify(&flight, 0, context(&airport, RING)),
        Some(AutoColumn::Awake)
    );
}

#[test]
fn an_arrival_without_a_position_is_not_displayed() {
    let airport = lfll();
    let flight = flight("AFR1234", "LFPG", "LFLL", None, &airport);
    assert_eq!(classify(&flight, 0, context(&airport, RING)), None);
}

#[test]
fn a_stale_position_counts_as_no_position() {
    let airport = lfll();
    let (lat, lon) = offset(0.0, 5.0);
    let flight = flight(
        "AFR1234",
        "LFPG",
        "LFLL",
        Some(Where::airborne(lat, lon, 4000)),
        &airport,
    );

    assert_eq!(
        classify(&flight, MAX_POSITION_AGE, context(&airport, RING)),
        Some(AutoColumn::Arrival)
    );
    assert_eq!(
        classify(&flight, MAX_POSITION_AGE + 1, context(&airport, RING)),
        None
    );
}

#[test]
fn an_arrival_inside_the_ring_is_an_arrival_and_outside_it_is_nothing() {
    let airport = lfll();
    let (near_lat, near_lon) = offset(180.0, 10.0);
    let near = flight(
        "AFR1234",
        "LFPG",
        "LFLL",
        Some(Where::airborne(near_lat, near_lon, 6000)),
        &airport,
    );
    assert_eq!(
        classify(&near, 0, context(&airport, RING)),
        Some(AutoColumn::Arrival)
    );

    let (far_lat, far_lon) = offset(180.0, 60.0);
    let far = flight(
        "AFR5678",
        "LFPG",
        "LFLL",
        Some(Where::airborne(far_lat, far_lon, 20000)),
        &airport,
    );
    assert_eq!(classify(&far, 0, context(&airport, RING)), None);
}

#[test]
fn unrelated_traffic_inside_the_ring_is_a_transit() {
    let airport = lfll();
    let (lat, lon) = offset(270.0, 12.0);
    let inside = flight(
        "FGEKO",
        "LFNE",
        "LFMU",
        Some(Where::airborne(lat, lon, 8000)),
        &airport,
    );
    assert_eq!(
        classify(&inside, 0, context(&airport, RING)),
        Some(AutoColumn::Transit)
    );

    let (far_lat, far_lon) = offset(270.0, 40.0);
    let outside = flight(
        "FGTJR",
        "LFNE",
        "LFMU",
        Some(Where::airborne(far_lat, far_lon, 8000)),
        &airport,
    );
    assert_eq!(classify(&outside, 0, context(&airport, RING)), None);
}

#[test]
fn the_ring_boundary_is_inclusive() {
    let airport = lfll();
    let (lat, lon) = offset(45.0, 20.0);
    let on_the_ring = flight(
        "AFR1234",
        "LFPG",
        "LFLL",
        Some(Where::airborne(lat, lon, 6000)),
        &airport,
    );
    let exact = on_the_ring.distance_nm.expect("distance is known");

    assert_eq!(
        classify(&on_the_ring, 0, context(&airport, exact)),
        Some(AutoColumn::Arrival),
        "an aircraft exactly on the ring is inside it"
    );
    assert_eq!(
        classify(&on_the_ring, 0, context(&airport, exact - 1e-9)),
        None,
        "a hair outside the ring is outside it"
    );
}

#[test]
fn a_flight_both_to_and_from_the_field_is_treated_as_a_departure() {
    let airport = lfll();
    let (lat, lon) = offset(90.0, 3.0);
    let circuit = flight(
        "FGEKO",
        "LFLL",
        "LFLL",
        Some(Where::airborne(lat, lon, 2500)),
        &airport,
    );
    assert_eq!(
        classify(&circuit, 0, context(&airport, RING)),
        Some(AutoColumn::Awake)
    );
}

#[test]
fn a_departure_is_awake_regardless_of_distance() {
    let airport = lfll();
    let (lat, lon) = offset(0.0, 45.0);
    let outbound = flight(
        "AFR1234",
        "LFLL",
        "LFPG",
        Some(Where::airborne(lat, lon, 25000)),
        &airport,
    );
    assert_eq!(
        classify(&outbound, 0, context(&airport, RING)),
        Some(AutoColumn::Awake),
        "distance does not declassify a departure; the removal rules retire it"
    );
}

#[test]
fn the_icao_comparison_ignores_case() {
    let airport = lfll();
    let flight = flight("AFR1234", "lfll", "LFPG", None, &airport);
    assert_eq!(
        classify(&flight, 0, context(&airport, RING)),
        Some(AutoColumn::Awake)
    );
}
