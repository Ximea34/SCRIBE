use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mock_aurora::{MockAurora, Script, Traffic};

const CENTRE_LAT: f64 = 45.725556;
const CENTRE_LON: f64 = 5.081111;
const FIELD_ELEVATION: i32 = 821;
const TICK: Duration = Duration::from_secs(1);
const AIRLINES: [&str; 8] = ["AFR", "RYR", "EZY", "DLH", "BAW", "VLG", "TVF", "SWR"];

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let count = number(&args, "--traffics").unwrap_or(50);
    let port = number(&args, "--port").unwrap_or(1130) as u16;

    let script = Arc::new(Mutex::new(
        Script::new("LFLL_TWR").with_traffics(fleet(count)),
    ));
    let server = MockAurora::bind(
        SocketAddr::from(([127, 0, 0, 1], port)),
        Arc::clone(&script),
    )
    .await?;
    println!(
        "mock Aurora listening on {} with {count} traffics",
        server.addr()
    );

    let mut ticker = tokio::time::interval(TICK);
    loop {
        ticker.tick().await;
        if let Ok(mut script) = script.lock() {
            advance(&mut script.traffics);
        }
    }
}

fn number(args: &[String], name: &str) -> Option<u32> {
    let index = args.iter().position(|arg| arg == name)?;
    args.get(index + 1)?.parse().ok()
}

fn fleet(count: u32) -> Vec<Traffic> {
    (0..count).map(build).collect()
}

fn build(index: u32) -> Traffic {
    let noise = mix(index);
    let bearing = f64::from(noise % 360);
    let distance = 0.5 + f64::from(noise / 360 % 400) / 10.0;
    let (lat, lon) = offset(CENTRE_LAT, CENTRE_LON, bearing, distance);
    let airline = AIRLINES[noise as usize % AIRLINES.len()];
    let callsign = format!("{airline}{:04}", 1000 + index % 9000);

    let mut traffic = match index % 3 {
        0 => Traffic::new(&callsign).route("LFLL", "LFPG"),
        1 => Traffic::new(&callsign).route("LFPG", "LFLL").rules("I"),
        _ => Traffic::new(&callsign).route("LFBO", "LFSB").rules("V"),
    }
    .at(lat, lon, FIELD_ELEVATION + (noise % 340) as i32 * 100)
    .eobt(&format!("{:02}{:02}", noise / 7 % 24, noise / 3 % 12 * 5));

    traffic.heading = bearing as u16;
    traffic.track = traffic.heading;
    traffic.ground_speed = 140 + (noise % 300) as u16;
    traffic.vertical_speed = (noise % 3000) as i32 - 1500;
    traffic.squawk_set = format!("{:04}", 1000 + noise % 6000);
    traffic.squawk_label = traffic.squawk_set.clone();

    if distance < 1.5 {
        traffic.on_ground = true;
        traffic.ground_speed = 0;
        traffic.altitude = FIELD_ELEVATION;
        traffic.vertical_speed = 0;
    }
    traffic
}

fn advance(traffics: &mut [Traffic]) {
    for traffic in traffics.iter_mut() {
        if traffic.on_ground {
            continue;
        }
        let nautical_miles = f64::from(traffic.ground_speed) / 3600.0;
        let (lat, lon) = offset(
            traffic.lat,
            traffic.lon,
            f64::from(traffic.heading),
            nautical_miles,
        );
        traffic.lat = lat;
        traffic.lon = lon;
        traffic.altitude = (traffic.altitude + traffic.vertical_speed / 60).clamp(0, 45_000);
    }
}

fn offset(lat: f64, lon: f64, bearing_degrees: f64, nautical_miles: f64) -> (f64, f64) {
    let bearing = bearing_degrees.to_radians();
    let delta_lat = nautical_miles * bearing.cos() / 60.0;
    let delta_lon = nautical_miles * bearing.sin() / (60.0 * lat.to_radians().cos());
    (lat + delta_lat, lon + delta_lon)
}

fn mix(index: u32) -> u32 {
    let mut x = index.wrapping_add(1).wrapping_mul(2_654_435_761);
    x ^= x >> 15;
    x = x.wrapping_mul(2_246_822_519);
    x ^ (x >> 13)
}
