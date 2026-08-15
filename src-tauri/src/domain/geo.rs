/// Mean Earth radius in nautical miles.
pub const EARTH_RADIUS_NM: f64 = 3440.065;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

impl LatLon {
    pub fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }
}

/// Great-circle distance in nautical miles, via the haversine formula.
pub fn distance_nm(from: LatLon, to: LatLon) -> f64 {
    let from_lat = from.lat.to_radians();
    let to_lat = to.lat.to_radians();
    let delta_lat = (to.lat - from.lat).to_radians();
    let delta_lon = (to.lon - from.lon).to_radians();

    let chord = (delta_lat / 2.0).sin().powi(2)
        + from_lat.cos() * to_lat.cos() * (delta_lon / 2.0).sin().powi(2);
    2.0 * chord.sqrt().clamp(0.0, 1.0).asin() * EARTH_RADIUS_NM
}
