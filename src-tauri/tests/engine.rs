mod common;

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mock_aurora::{MockAurora, Script, Traffic};
use scribe_lib::aurora::AuroraClient;
use scribe_lib::domain::{BoardUpdate, Store};
use scribe_lib::engine::{self, BoardSink, EngineOptions};
use scribe_lib::settings::Settings;

use common::{lfll, offset};

const FLEET: u32 = 200;
/// Enough for every flight plan to be fetched once, which is a one-off cost.
const WARM_UP: Duration = Duration::from_millis(4_000);
/// Long enough that the 4 s far-traffic cadence must have come round at least twice.
const OBSERVATION: Duration = Duration::from_millis(9_000);

#[derive(Default)]
struct Recorder {
    updates: Mutex<Vec<BoardUpdate>>,
}

struct RecordingSink(Arc<Recorder>);

impl BoardSink for RecordingSink {
    fn emit(&self, update: BoardUpdate) {
        if let Ok(mut updates) = self.0.updates.lock() {
            updates.push(update);
        }
    }
}

/// A plausible busy event: a third waiting on the apron to depart, a third inbound between 2 and
/// 40 NM, a third transiting between 5 and 45 NM. Every priority class is exercised and nothing
/// qualifies for removal, so the whole fleet stays trackable for the duration.
fn fleet() -> Vec<Traffic> {
    (0..FLEET)
        .map(|index| {
            let bearing = f64::from(index % 120) * 3.0;
            let callsign = format!("TFC{index:04}");
            let eobt = format!("{:02}{:02}", index % 24, index % 12 * 5);

            match index % 3 {
                0 => {
                    let (lat, lon) = offset(bearing, 0.3);
                    Traffic::new(&callsign)
                        .route("LFLL", "LFPG")
                        .eobt(&eobt)
                        .at(lat, lon, 821)
                        .on_ground(true)
                }
                1 => {
                    let (lat, lon) = offset(bearing, 2.0 + f64::from(index % 38));
                    Traffic::new(&callsign)
                        .route("LFPG", "LFLL")
                        .eobt(&eobt)
                        .at(lat, lon, 3_000 + (index as i32 % 30) * 500)
                }
                _ => {
                    let (lat, lon) = offset(bearing, 5.0 + f64::from(index % 40));
                    Traffic::new(&callsign)
                        .route("LFBO", "LFSB")
                        .eobt(&eobt)
                        .at(lat, lon, 8_000 + (index as i32 % 25) * 800)
                }
            }
        })
        .collect()
}

fn settings(addr: std::net::SocketAddr) -> Settings {
    Settings {
        aurora_host: addr.ip(),
        aurora_port: addr.port(),
        selected_icao: Some("LFLL".to_owned()),
        ..Settings::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_board_fills_and_stays_inside_its_polling_budget_at_two_hundred_traffics() {
    let server = MockAurora::start(Script::new("LFLL_TWR").with_traffics(fleet()))
        .await
        .expect("mock should bind");
    let stats = server.stats();
    let settings = settings(server.addr());

    let recorder = Arc::new(Recorder::default());
    let store = Store::new(lfll(), settings.domain_config());
    let (client, _client_task) = AuroraClient::spawn(settings.client_config());
    let (engine, task) = engine::spawn(
        client,
        store,
        EngineOptions {
            settings: settings.clone(),
            activations_path: std::env::temp_dir().join("scribe-engine-test-activations.json"),
        },
        Box::new(RecordingSink(Arc::clone(&recorder))),
    );

    tokio::time::sleep(WARM_UP).await;
    let warm_up_plans = stats.flight_plans.load(Ordering::Relaxed);
    stats.reset();

    let started = Instant::now();
    tokio::time::sleep(OBSERVATION).await;
    let elapsed = started.elapsed().as_secs_f64();
    engine.shutdown().await;
    let _ = task.await;

    let requests = stats.requests.load(Ordering::Relaxed);
    let positions = stats.positions.load(Ordering::Relaxed);
    let plans = stats.flight_plans.load(Ordering::Relaxed);
    let observed_rate = requests as f64 / elapsed;
    let budget = f64::from(settings.polling.budget_requests_per_second);

    let snapshot = {
        let updates = recorder.updates.lock().expect("updates");
        updates
            .iter()
            .rev()
            .find_map(|update| update.columns.clone())
            .expect("the board should have been published at least once")
    };
    let on_board = snapshot.awake.len()
        + snapshot.activated_departures.len()
        + snapshot.arrivals.len()
        + snapshot.transits.len();

    println!(
        "\n--- steady state, {FLEET} traffics over {elapsed:.1} s ---\n\
         requests       {requests} ({observed_rate:.1}/s, budget {budget:.0}/s)\n\
         positions      {positions}\n\
         flight plans   {plans} during the window, {warm_up_plans} on cold start\n\
         swept          {} of {FLEET} callsigns, worst served {}x\n\
         on the board   {on_board} strips\n\
         board updates  {}\n",
        stats.distinct_callsigns_polled(),
        stats.least_polled(),
        recorder.updates.lock().map_or(0, |updates| updates.len())
    );

    assert!(
        observed_rate <= budget * 1.15,
        "the budget must hold: {observed_rate:.1}/s against {budget:.0}/s"
    );
    assert_eq!(
        warm_up_plans,
        u64::from(FLEET),
        "each flight plan is fetched exactly once on start-up"
    );
    assert_eq!(
        plans, 0,
        "a cached flight plan must not be refetched inside its TTL"
    );
    assert_eq!(
        stats.distinct_callsigns_polled(),
        FLEET as usize,
        "every aircraft must be reached, not just the ones on the board"
    );
    assert!(
        stats.least_polled() >= 2,
        "the worst-served aircraft was polled {}x in {elapsed:.1} s; the target is at least \
         once every 4 s",
        stats.least_polled()
    );
    assert!(on_board > 0, "the board should not be empty");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_board_stays_empty_and_quiet_when_aurora_is_unreachable() {
    let settings = settings(std::net::SocketAddr::from(([127, 0, 0, 1], 1)));
    let recorder = Arc::new(Recorder::default());
    let store = Store::new(lfll(), settings.domain_config());
    let (client, _client_task) = AuroraClient::spawn(settings.client_config());
    let (engine, task) = engine::spawn(
        client,
        store,
        EngineOptions {
            settings,
            activations_path: std::env::temp_dir().join("scribe-engine-offline-activations.json"),
        },
        Box::new(RecordingSink(Arc::clone(&recorder))),
    );

    tokio::time::sleep(Duration::from_millis(500)).await;
    let snapshot = engine.snapshot().await.expect("the engine answers");
    engine.shutdown().await;
    let _ = task.await;

    assert!(snapshot.columns.callsigns().next().is_none());
    assert!(snapshot.strips.is_empty());
    assert_eq!(
        recorder.updates.lock().map_or(1, |updates| updates.len()),
        0,
        "an empty board is not a change worth emitting"
    );
}
