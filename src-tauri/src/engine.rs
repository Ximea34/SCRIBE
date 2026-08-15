use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;
use tracing::{debug, info, warn};

use crate::aurora::scheduler::{Priority, Scheduler, Task};
use crate::aurora::types::{FlightPlan, TrafficPosition};
use crate::aurora::{AuroraClient, AuroraError, ConnectionState};
use crate::domain::activations;
use crate::domain::{BoardSnapshot, BoardUpdate, Column, Millis, Store};
use crate::ipc::{FlightDetail, IpcError};
use crate::settings::Settings;

const COMMAND_CAPACITY: usize = 64;
const RESULT_CAPACITY: usize = 512;
/// Traffic within this multiple of the ring is polled faster, so entries are noticed early.
/// At 20 NM this is a 30 NM watch band: roughly two minutes' warning at 300 kt.
const NEAR_RING_FACTOR: f64 = 1.5;
const SECONDS_PER_DAY: u64 = 86_400;

/// Where board updates go. A trait so the engine can run without Tauri in tests.
pub trait BoardSink: Send + 'static {
    fn emit(&self, update: BoardUpdate);
}

/// Monotonic milliseconds for the domain, wall clock only where UTC actually matters.
pub struct Clock {
    origin: Instant,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Clock {
    pub fn now(&self) -> Millis {
        self.origin.elapsed().as_millis() as u64
    }

    pub fn utc_minutes(&self) -> u16 {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        ((seconds % SECONDS_PER_DAY) / 60) as u16
    }

    pub fn unix_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as u64)
    }
}

enum Command {
    Snapshot(oneshot::Sender<BoardSnapshot>),
    Activate {
        callsign: String,
        respond_to: oneshot::Sender<Result<(), IpcError>>,
    },
    Detail {
        callsign: String,
        respond_to: oneshot::Sender<Option<FlightDetail>>,
    },
    Shutdown(oneshot::Sender<()>),
}

enum Ingest {
    TrafficList(Result<Vec<Box<str>>, AuroraError>),
    Station(Result<Box<str>, AuroraError>),
    FlightPlan {
        callsign: Box<str>,
        result: Result<FlightPlan, AuroraError>,
    },
    Position {
        callsign: Box<str>,
        result: Result<TrafficPosition, AuroraError>,
    },
}

#[derive(Debug, Clone)]
pub struct EngineHandle {
    commands: mpsc::Sender<Command>,
}

impl EngineHandle {
    pub async fn snapshot(&self) -> Result<BoardSnapshot, IpcError> {
        let (respond_to, response) = oneshot::channel();
        self.send(Command::Snapshot(respond_to)).await?;
        response.await.map_err(|_| IpcError::NotRunning)
    }

    pub async fn activate(&self, callsign: String) -> Result<(), IpcError> {
        let (respond_to, response) = oneshot::channel();
        self.send(Command::Activate {
            callsign,
            respond_to,
        })
        .await?;
        response.await.map_err(|_| IpcError::NotRunning)?
    }

    pub async fn detail(&self, callsign: String) -> Result<Option<FlightDetail>, IpcError> {
        let (respond_to, response) = oneshot::channel();
        self.send(Command::Detail {
            callsign,
            respond_to,
        })
        .await?;
        response.await.map_err(|_| IpcError::NotRunning)
    }

    pub async fn shutdown(&self) {
        let (respond_to, response) = oneshot::channel();
        if self.send(Command::Shutdown(respond_to)).await.is_ok() {
            let _ = response.await;
        }
    }

    async fn send(&self, command: Command) -> Result<(), IpcError> {
        self.commands
            .send(command)
            .await
            .map_err(|_| IpcError::NotRunning)
    }
}

pub struct EngineOptions {
    pub settings: Settings,
    pub activations_path: PathBuf,
}

struct Engine {
    client: AuroraClient,
    store: Store,
    scheduler: Scheduler,
    clock: Clock,
    sink: Box<dyn BoardSink>,
    settings: Settings,
    results: mpsc::Sender<Ingest>,
    activations_path: PathBuf,
    saved_activations: Vec<Box<str>>,
    station: Option<Box<str>>,
}

pub fn spawn(
    client: AuroraClient,
    store: Store,
    options: EngineOptions,
    sink: Box<dyn BoardSink>,
) -> (EngineHandle, JoinHandle<()>) {
    let (commands_tx, commands_rx) = mpsc::channel(COMMAND_CAPACITY);
    let (results_tx, results_rx) = mpsc::channel(RESULT_CAPACITY);
    let clock = Clock::default();
    let scheduler = Scheduler::new(options.settings.scheduler_config(), clock.now());

    let mut engine = Engine {
        client,
        store,
        scheduler,
        clock,
        sink,
        settings: options.settings,
        results: results_tx,
        activations_path: options.activations_path,
        saved_activations: Vec::new(),
        station: None,
    };
    engine.restore_activations();

    let join = tokio::spawn(run(engine, commands_rx, results_rx));
    (
        EngineHandle {
            commands: commands_tx,
        },
        join,
    )
}

async fn run(
    mut engine: Engine,
    mut commands: mpsc::Receiver<Command>,
    mut results: mpsc::Receiver<Ingest>,
) {
    let mut tick = tokio::time::interval(Duration::from_millis(
        engine.settings.polling.emit_interval_ms.max(1),
    ));
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut connection = engine.client.watch_state();
    info!("board engine started");

    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::Shutdown(respond_to)) => {
                    let _ = respond_to.send(());
                    break;
                }
                Some(command) => engine.handle(command),
                None => break,
            },
            Some(ingest) = results.recv() => engine.ingest(ingest),
            _ = tick.tick() => {
                engine.dispatch();
                engine.publish();
            },
            changed = connection.changed() => {
                if changed.is_ok() {
                    let state = *connection.borrow_and_update();
                    engine.connection_changed(state);
                }
            },
        }
    }

    engine.save_activations(true);
    info!("board engine stopped");
}

impl Engine {
    fn handle(&mut self, command: Command) {
        match command {
            Command::Snapshot(respond_to) => {
                let _ = respond_to.send(self.store.snapshot());
            }
            Command::Activate {
                callsign,
                respond_to,
            } => {
                let now = self.clock.now();
                let outcome = self.store.activate(&callsign, now).map_err(IpcError::from);
                if outcome.is_ok() {
                    // A click must land immediately rather than wait for the next tick.
                    self.publish();
                    self.save_activations(true);
                }
                let _ = respond_to.send(outcome);
            }
            Command::Detail {
                callsign,
                respond_to,
            } => {
                let detail = self.store.flight(&callsign).map(FlightDetail::from);
                let _ = respond_to.send(detail);
            }
            Command::Shutdown(respond_to) => {
                let _ = respond_to.send(());
            }
        }
    }

    fn ingest(&mut self, ingest: Ingest) {
        let now = self.clock.now();
        match ingest {
            Ingest::TrafficList(Ok(callsigns)) => {
                self.store
                    .observe_radar(callsigns.iter().map(|callsign| &**callsign), now);
            }
            Ingest::TrafficList(Err(error)) => debug!(%error, "traffic list unavailable"),
            Ingest::Station(Ok(station)) => {
                if self.station.as_deref() != Some(&*station) {
                    info!(%station, "controlling station");
                    self.station = Some(station);
                }
            }
            Ingest::Station(Err(error)) => debug!(%error, "station unavailable"),
            Ingest::FlightPlan { callsign, result } => {
                self.scheduler.completed_flight_plan(&callsign);
                match result {
                    Ok(plan) => self.store.observe_flight_plan(&callsign, plan, now),
                    Err(error) => {
                        debug!(%callsign, %error, "no flight plan");
                        self.store.observe_missing_flight_plan(&callsign, now);
                    }
                }
            }
            Ingest::Position { callsign, result } => match result {
                Ok(position) => {
                    self.scheduler.completed_position(&callsign);
                    self.store.observe_position(&callsign, position, now);
                }
                Err(error) => {
                    debug!(%callsign, %error, "position unavailable");
                    self.scheduler.penalise(&callsign);
                }
            },
        }
    }

    fn dispatch(&mut self) {
        let now = self.clock.now();
        self.sync_scheduler(now);
        let plans = self
            .store
            .callsigns_needing_flight_plan(now, self.settings.polling.flight_plan_ttl_ms);
        for task in self.scheduler.take_due(now, &plans) {
            self.issue(task);
        }
    }

    fn sync_scheduler(&mut self, now: Millis) {
        let near_ring = self.settings.ring_radius_nm * NEAR_RING_FACTOR;
        let max_age = self.settings.polling.max_position_age_ms;
        let Self {
            store, scheduler, ..
        } = self;

        for flight in store.flights() {
            if flight.state.is_archived() {
                continue;
            }
            // A departure's order comes from its EOBT, so its position only settles removal —
            // polling a parked strip every second buys nothing and costs the whole budget.
            let priority = match flight.state.column() {
                Some(Column::Arrival | Column::Transit) => Priority::Board,
                Some(_) => Priority::Near,
                None if flight
                    .fresh_distance_nm(now, max_age)
                    .is_some_and(|nautical_miles| nautical_miles <= near_ring) =>
                {
                    Priority::Near
                }
                None => Priority::Far,
            };
            scheduler.observe(&flight.callsign, priority);
        }
        scheduler.retain(|callsign| {
            store
                .flight(callsign)
                .is_some_and(|flight| !flight.state.is_archived())
        });
    }

    fn issue(&self, task: Task) {
        let client = self.client.clone();
        let results = self.results.clone();
        tokio::spawn(async move {
            let ingest = match task {
                Task::TrafficList => Ingest::TrafficList(client.traffic_list().await),
                Task::Station => Ingest::Station(client.conn().await),
                Task::FlightPlan(callsign) => {
                    let result = client.flight_plan(&callsign).await;
                    Ingest::FlightPlan { callsign, result }
                }
                Task::Position(callsign) => {
                    let result = client.traffic_position(&callsign).await;
                    Ingest::Position { callsign, result }
                }
            };
            let _ = results.send(ingest).await;
        });
    }

    fn publish(&mut self) {
        let now = self.clock.now();
        self.store.apply(now, self.clock.utc_minutes());
        if let Some(update) = self.store.take_update() {
            self.sink.emit(update);
            self.save_activations(false);
        }
    }

    fn connection_changed(&mut self, state: ConnectionState) {
        if state == ConnectionState::Connected {
            let now = self.clock.now();
            self.scheduler.reset(now);
            self.store.rearm_flight_plans(now);
            info!("Aurora connected; refreshing the whole board");
        }
    }

    fn restore_activations(&mut self) {
        let records = activations::load(&self.activations_path);
        if records.is_empty() {
            return;
        }
        let restored = self
            .store
            .restore_from(records, self.clock.unix_ms(), self.clock.now());
        info!(restored, "activations carried over from the last session");
    }

    /// Only writes when the activated set actually changed, off the board's thread.
    fn save_activations(&mut self, force: bool) {
        let current = self.store.board().columns.activated_departures.clone();
        if !force && current == self.saved_activations {
            return;
        }
        self.saved_activations = current;

        let records = self
            .store
            .activation_records(self.clock.unix_ms(), self.clock.now());
        let path = self.activations_path.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) = activations::save(&path, &records) {
                warn!(%error, "cannot persist activations");
            }
        });
    }
}
