mod common;

use common::{lfll, offset, plan, position, Where};
use scribe_lib::domain::ordering::{altitude_key, distance_key, eobt_sort_key, order, parse_eobt};
use scribe_lib::domain::{Column, Flight};

const MIDNIGHT: u16 = 0;
const NOON: u16 = 12 * 60;

fn departure(callsign: &str, eobt: &str) -> Flight {
    Flight::new(callsign, plan(callsign, "LFLL", "LFPG", eobt, "I"), 0)
}

fn inbound(callsign: &str, nautical_miles: f64, altitude: i32) -> Flight {
    let airport = lfll();
    let (lat, lon) = offset(0.0, nautical_miles);
    let mut flight = Flight::new(callsign, plan(callsign, "LFPG", "LFLL", "1200", "I"), 0);
    flight.set_position(
        position(callsign, &Where::airborne(lat, lon, altitude)),
        airport.centre(),
        0,
    );
    flight
}

fn sorted(column: Column, flights: &[Flight], now_minutes: u16) -> Vec<&str> {
    let mut refs: Vec<&Flight> = flights.iter().collect();
    order(column, &mut refs, now_minutes);
    refs.iter().map(|flight| &*flight.callsign).collect()
}

#[test]
fn eobt_accepts_the_filed_form_and_rejects_the_rest() {
    assert_eq!(parse_eobt("1215"), Some(735));
    assert_eq!(parse_eobt("12:15"), Some(735));
    assert_eq!(parse_eobt(" 0000 "), Some(0));
    assert_eq!(parse_eobt("2359"), Some(1439));
    assert_eq!(parse_eobt(""), None);
    assert_eq!(parse_eobt("2400"), None);
    assert_eq!(parse_eobt("1260"), None);
    assert_eq!(parse_eobt("121"), None);
    assert_eq!(parse_eobt("12155"), None);
    assert_eq!(parse_eobt("abcd"), None);
}

#[test]
fn eobt_ordering_wraps_around_midnight() {
    let late = eobt_sort_key(parse_eobt("2350"), MIDNIGHT);
    let early = eobt_sort_key(parse_eobt("0010"), MIDNIGHT);
    assert!(late < early, "2350 is overdue, 0010 is upcoming");
    assert_eq!(late, -10);
    assert_eq!(early, 10);
}

#[test]
fn a_missing_eobt_always_sorts_last() {
    assert_eq!(eobt_sort_key(None, NOON), i32::MAX);
    assert!(eobt_sort_key(parse_eobt("2359"), NOON) < eobt_sort_key(None, NOON));
}

#[test]
fn awake_departures_sort_by_eobt_then_callsign() {
    let flights = [
        departure("RYR33EK", "1230"),
        departure("AFR1234", "1200"),
        departure("EZY9999", ""),
        departure("BAW0001", "1200"),
    ];
    assert_eq!(
        sorted(Column::Awake, &flights, NOON),
        ["AFR1234", "BAW0001", "RYR33EK", "EZY9999"]
    );
}

#[test]
fn activated_departures_use_the_same_eobt_order() {
    let flights = [departure("BBB1111", "1300"), departure("AAA2222", "1210")];
    assert_eq!(
        sorted(Column::ActivatedDeparture, &flights, NOON),
        ["AAA2222", "BBB1111"]
    );
}

#[test]
fn arrivals_sort_closest_first_then_lowest() {
    let flights = [
        inbound("FAR0001", 18.0, 9000),
        inbound("NEAR0001", 4.0, 3000),
        inbound("MID0001", 11.0, 6000),
    ];
    assert_eq!(
        sorted(Column::Arrival, &flights, NOON),
        ["NEAR0001", "MID0001", "FAR0001"]
    );
}

#[test]
fn arrivals_at_the_same_distance_break_the_tie_on_altitude_then_callsign() {
    let flights = [
        inbound("ZZZ0001", 8.0, 5000),
        inbound("AAA0001", 8.0, 5000),
        inbound("BBB0001", 8.0, 2000),
    ];
    assert_eq!(
        sorted(Column::Arrival, &flights, NOON),
        ["BBB0001", "AAA0001", "ZZZ0001"]
    );
}

#[test]
fn ground_traffic_sorts_to_the_top_of_the_arrivals() {
    let airport = lfll();
    let mut parked = Flight::new("FGEKO", plan("FGEKO", "LFPG", "LFLL", "1200", "I"), 0);
    parked.set_position(
        position(
            "FGEKO",
            &Where::grounded(common::LFLL_LAT, common::LFLL_LON),
        ),
        airport.centre(),
        0,
    );
    let flights = [inbound("AFR1234", 6.0, 4000), parked];

    assert_eq!(
        sorted(Column::Arrival, &flights, NOON),
        ["FGEKO", "AFR1234"],
        "on the field is both nearest and lowest"
    );
}

#[test]
fn transits_sort_on_distance_alone() {
    let flights = [
        inbound("FAR0001", 15.0, 2000),
        inbound("NEAR0001", 3.0, 30000),
    ];
    assert_eq!(
        sorted(Column::Transit, &flights, NOON),
        ["NEAR0001", "FAR0001"]
    );
}

#[test]
fn unknown_distances_and_altitudes_sort_last_without_panicking() {
    assert_eq!(distance_key(None), i32::MAX);
    assert_eq!(distance_key(Some(f64::NAN)), i32::MAX);
    assert_eq!(distance_key(Some(f64::INFINITY)), i32::MAX);
    assert_eq!(distance_key(Some(-1.0)), 0);
    assert_eq!(distance_key(Some(12.3456)), 12_346);
    assert_eq!(altitude_key(None), i32::MAX);
    assert_eq!(altitude_key(Some(-200)), -200);
}

#[test]
fn a_flight_with_no_position_sorts_after_every_positioned_one() {
    let no_position = Flight::new("NOPOS01", plan("NOPOS01", "LFPG", "LFLL", "1200", "I"), 0);
    let flights = [no_position, inbound("AFR1234", 19.0, 9000)];
    assert_eq!(
        sorted(Column::Arrival, &flights, NOON),
        ["AFR1234", "NOPOS01"]
    );
}
