use scribe_lib::airports::coords::{self, Axis, CoordError};

const TOLERANCE: f64 = 1e-6;

fn lat(input: &str) -> f64 {
    coords::parse(input, Axis::Latitude).unwrap_or_else(|e| panic!("{input:?} should parse: {e}"))
}

fn lon(input: &str) -> f64 {
    coords::parse(input, Axis::Longitude).unwrap_or_else(|e| panic!("{input:?} should parse: {e}"))
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < TOLERANCE
}

#[test]
fn the_lfll_fixture_parses_to_the_documented_values() {
    assert!(close(lat("45°43'32\"N"), 45.725556));
    assert!(close(lon("005°04'52\"E"), 5.081111));
}

#[test]
fn accepts_dms_with_symbols() {
    assert!(close(lat("45°43'32\"N"), 45.725556));
    assert!(close(lat("45º43′32″N"), 45.725556));
    assert!(close(lon("5°0'0\"W"), -5.0));
    assert!(close(lat("45°30'0\"S"), -45.5));
}

#[test]
fn accepts_ascii_dms_with_the_hemisphere_on_either_side() {
    assert!(close(lat("N45 43 32"), 45.725556));
    assert!(close(lat("45 43 32 N"), 45.725556));
    assert!(close(lat("N045.43.32"), 45.725556));
    assert!(close(lon("W005 04 52"), -5.081111));
    assert!(close(lon("005 04 52 W"), -5.081111));
}

#[test]
fn accepts_decimal_degrees_signed_or_with_a_hemisphere() {
    assert!(close(lat("45.725556"), 45.725556));
    assert!(close(lon("-5.081111"), -5.081111));
    assert!(close(lon("5.081111E"), 5.081111));
    assert!(close(lon("5.081111W"), -5.081111));
    assert!(close(lat("45"), 45.0));
    assert!(close(lat("+45.5"), 45.5));
}

#[test]
fn accepts_fractional_seconds_and_decimal_minutes() {
    assert!(close(
        lat("45 43 32.5 N"),
        45.0 + 43.0 / 60.0 + 32.5 / 3600.0
    ));
    assert!(close(lat("45 43.5 N"), 45.0 + 43.5 / 60.0));
    assert!(close(lat("45:43:32N"), 45.725556));
}

#[test]
fn tolerates_surrounding_whitespace_and_lower_case() {
    assert!(close(lat("  45°43'32\"n  "), 45.725556));
    assert!(close(lon(" w005 04 52 "), -5.081111));
}

#[test]
fn zero_and_the_poles_are_representable() {
    assert!(close(lat("0"), 0.0));
    assert!(close(lat("90N"), 90.0));
    assert!(close(lat("90S"), -90.0));
    assert!(close(lon("180E"), 180.0));
    assert!(close(lon("180W"), -180.0));
}

#[test]
fn rejects_an_empty_coordinate() {
    assert_eq!(coords::parse("", Axis::Latitude), Err(CoordError::Empty));
    assert_eq!(coords::parse("   ", Axis::Latitude), Err(CoordError::Empty));
}

#[test]
fn rejects_a_hemisphere_from_the_wrong_axis() {
    assert_eq!(
        coords::parse("45.0E", Axis::Latitude),
        Err(CoordError::WrongHemisphere {
            axis: Axis::Latitude,
            hemisphere: 'E'
        })
    );
    assert_eq!(
        coords::parse("N5.0", Axis::Longitude),
        Err(CoordError::WrongHemisphere {
            axis: Axis::Longitude,
            hemisphere: 'N'
        })
    );
}

#[test]
fn rejects_a_sign_together_with_a_hemisphere() {
    assert_eq!(
        coords::parse("-45 43 32 N", Axis::Latitude),
        Err(CoordError::SignAndHemisphere)
    );
}

#[test]
fn rejects_out_of_range_values() {
    assert!(matches!(
        coords::parse("91", Axis::Latitude),
        Err(CoordError::OutOfRange { .. })
    ));
    assert!(matches!(
        coords::parse("180.5", Axis::Longitude),
        Err(CoordError::OutOfRange { .. })
    ));
    assert!(matches!(
        coords::parse("90 00 01 N", Axis::Latitude),
        Err(CoordError::OutOfRange { .. })
    ));
}

#[test]
fn rejects_impossible_minutes_and_seconds() {
    assert_eq!(
        coords::parse("45 60 00 N", Axis::Latitude),
        Err(CoordError::Minutes(60.0))
    );
    assert_eq!(
        coords::parse("45 30 60 N", Axis::Latitude),
        Err(CoordError::Seconds(60.0))
    );
}

#[test]
fn rejects_nonsense() {
    assert!(matches!(
        coords::parse("north", Axis::Latitude),
        Err(CoordError::Malformed(_))
    ));
    assert!(matches!(
        coords::parse("45 xx 32 N", Axis::Latitude),
        Err(CoordError::Malformed(_))
    ));
    assert!(matches!(
        coords::parse("45..32", Axis::Latitude),
        Err(CoordError::Malformed(_))
    ));
    assert!(matches!(
        coords::parse("45 43 32 11 N", Axis::Latitude),
        Err(CoordError::TooManyParts(_))
    ));
    assert!(matches!(
        coords::parse("N", Axis::Latitude),
        Err(CoordError::Malformed(_))
    ));
}
