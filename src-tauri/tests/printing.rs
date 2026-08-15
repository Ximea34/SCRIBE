mod common;

use common::{lfll, plan, position, Where, LFLL_LAT, LFLL_LON};
use scribe_lib::domain::Flight;
use scribe_lib::printing::StripDocument;

#[test]
fn a_strip_document_carries_the_plan_and_the_live_state() {
    let airport = lfll();
    let mut flight = Flight::new("AFR1234", plan("AFR1234", "LFLL", "LFPG", "1215", "I"), 0);
    flight.set_position(
        position("AFR1234", &Where::grounded(LFLL_LAT, LFLL_LON).gate("A12")),
        airport.centre(),
        0,
    );

    let document = StripDocument::from(&flight);
    assert_eq!(&*document.callsign, "AFR1234");
    assert_eq!(&*document.dep, "LFLL");
    assert_eq!(&*document.arr, "LFPG");
    assert_eq!(&*document.eobt, "1215");
    assert_eq!(&*document.rules, "I");
    assert_eq!(&*document.aircraft, "A320");
    assert_eq!(&*document.cruise_level, "F330");
    assert_eq!(&*document.squawk, "7000");
    assert_eq!(&*document.stand, "A12");
}

#[test]
fn a_strip_document_is_printable_before_any_position_arrives() {
    let flight = Flight::new("RYR33EK", plan("RYR33EK", "LFLL", "LFKJ", "0800", "I"), 0);

    let document = StripDocument::from(&flight);
    assert_eq!(&*document.callsign, "RYR33EK");
    assert_eq!(&*document.arr, "LFKJ");
    assert!(document.squawk.is_empty());
    assert!(document.stand.is_empty());
    assert!(document.assumed_by.is_empty());
}
