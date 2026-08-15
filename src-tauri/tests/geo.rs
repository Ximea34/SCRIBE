use scribe_lib::domain::geo::{distance_nm, LatLon, EARTH_RADIUS_NM};

const LFLL: LatLon = LatLon {
    lat: 45.725556,
    lon: 5.081111,
};
const LFPG: LatLon = LatLon {
    lat: 49.009722,
    lon: 2.547778,
};

#[test]
fn a_point_is_zero_from_itself() {
    assert_eq!(distance_nm(LFLL, LFLL), 0.0);
}

#[test]
fn one_degree_of_latitude_is_a_touch_over_sixty_nautical_miles() {
    let north = LatLon::new(LFLL.lat + 1.0, LFLL.lon);
    let expected = EARTH_RADIUS_NM * 1.0_f64.to_radians();
    assert!((distance_nm(LFLL, north) - expected).abs() < 1e-6);
    assert!((expected - 60.04).abs() < 0.01);
}

#[test]
fn one_degree_of_longitude_shrinks_with_latitude() {
    let at_equator = distance_nm(LatLon::new(0.0, 0.0), LatLon::new(0.0, 1.0));
    let at_lyon = distance_nm(LFLL, LatLon::new(LFLL.lat, LFLL.lon + 1.0));
    assert!((at_equator - 60.04).abs() < 0.01);
    assert!(at_lyon < at_equator * 0.75);
}

#[test]
fn the_distance_is_symmetric() {
    assert!((distance_nm(LFLL, LFPG) - distance_nm(LFPG, LFLL)).abs() < 1e-9);
}

#[test]
fn lyon_to_paris_matches_the_published_great_circle() {
    let distance = distance_nm(LFLL, LFPG);
    assert!(
        (215.0..230.0).contains(&distance),
        "expected roughly 222 NM, got {distance}"
    );
}

#[test]
fn antipodal_points_are_half_the_circumference() {
    let distance = distance_nm(LatLon::new(0.0, 0.0), LatLon::new(0.0, 180.0));
    assert!((distance - std::f64::consts::PI * EARTH_RADIUS_NM).abs() < 1e-6);
}
