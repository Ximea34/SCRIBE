use std::collections::VecDeque;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::{Instant, MissedTickBehavior};
use tracing::{debug, info, warn};

use super::codec::LineFramer;
use super::protocol::{self, Command, CommandName, Response};
use super::types::{AtcPosition, FlightPlan, TrafficPosition};
use super::AuroraError;

pub const DEFAULT_PORT: u16 = 1130;
pub const DEFAULT_ADDR: SocketAddr =
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, DEFAULT_PORT));

const REQUEST_CHANNEL_CAPACITY: usize = 256;
const READ_CHUNK: usize = 8192;
const TIMEOUT_SWEEP: Duration = Duration::from_millis(250);
const BACKOFF_SHIFT_CAP: u32 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    pub addr: SocketAddr,
    pub request_timeout: Duration,
    pub backoff_initial: Duration,
    pub backoff_max: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            addr: DEFAULT_ADDR,
            request_timeout: Duration::from_secs(2),
            backoff_initial: Duration::from_millis(250),
            backoff_max: Duration::from_secs(8),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting { attempt: u32 },
    Connected,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    Conn(Box<str>),
    SelTfc(Option<Box<str>>),
    FlightPlan(FlightPlan),
    TrafficPosition(TrafficPosition),
    TrafficList(Vec<Box<str>>),
    Atc(Vec<AtcPosition>),
}

/// Handle onto the owning connection task; cloning it is cheap and shares one socket.
#[derive(Debug, Clone)]
pub struct AuroraClient {
    requests: mpsc::Sender<Request>,
    state: watch::Receiver<ConnectionState>,
}

struct Request {
    command: Command,
    respond_to: oneshot::Sender<Result<Reply, AuroraError>>,
}

struct Pending {
    deadline: Instant,
    respond_to: oneshot::Sender<Result<Reply, AuroraError>>,
}

type Queues = [VecDeque<Pending>; CommandName::COUNT];

enum SessionEnd {
    PeerClosed,
    Io(std::io::Error),
    Shutdown,
}

impl AuroraClient {
    /// Spawns the connection task; it reconnects on its own until the returned client is dropped.
    pub fn spawn(config: ClientConfig) -> (Self, JoinHandle<()>) {
        let (requests_tx, requests_rx) = mpsc::channel(REQUEST_CHANNEL_CAPACITY);
        let (state_tx, state_rx) = watch::channel(ConnectionState::Disconnected);
        let join = tokio::spawn(run(config, requests_rx, state_tx));
        let client = Self {
            requests: requests_tx,
            state: state_rx,
        };
        (client, join)
    }

    pub fn state(&self) -> ConnectionState {
        *self.state.borrow()
    }

    pub fn watch_state(&self) -> watch::Receiver<ConnectionState> {
        self.state.clone()
    }

    pub async fn request(&self, command: Command) -> Result<Reply, AuroraError> {
        let (respond_to, response) = oneshot::channel();
        self.requests
            .send(Request {
                command,
                respond_to,
            })
            .await
            .map_err(|_| AuroraError::ClientStopped)?;
        response.await.map_err(|_| AuroraError::ClientStopped)?
    }

    pub async fn conn(&self) -> Result<Box<str>, AuroraError> {
        match self.request(Command::Conn).await? {
            Reply::Conn(station) => Ok(station),
            _ => Err(AuroraError::UnexpectedReply { expected: "CONN" }),
        }
    }

    /// Safe on its own; the `%SELTFC%` substitution that closes the socket is never sent (4.6.1).
    pub async fn selected(&self) -> Result<Option<Box<str>>, AuroraError> {
        match self.request(Command::SelTfc).await? {
            Reply::SelTfc(callsign) => Ok(callsign),
            _ => Err(AuroraError::UnexpectedReply { expected: "SELTFC" }),
        }
    }

    pub async fn flight_plan(&self, callsign: &str) -> Result<FlightPlan, AuroraError> {
        match self.request(Command::flight_plan(callsign)?).await? {
            Reply::FlightPlan(plan) => Ok(plan),
            _ => Err(AuroraError::UnexpectedReply { expected: "FP" }),
        }
    }

    pub async fn traffic_position(&self, callsign: &str) -> Result<TrafficPosition, AuroraError> {
        match self.request(Command::traffic_position(callsign)?).await? {
            Reply::TrafficPosition(position) => Ok(position),
            _ => Err(AuroraError::UnexpectedReply { expected: "TRPOS" }),
        }
    }

    pub async fn traffic_list(&self) -> Result<Vec<Box<str>>, AuroraError> {
        match self.request(Command::TrafficList).await? {
            Reply::TrafficList(callsigns) => Ok(callsigns),
            _ => Err(AuroraError::UnexpectedReply { expected: "TR" }),
        }
    }

    pub async fn atc(&self) -> Result<Vec<AtcPosition>, AuroraError> {
        match self.request(Command::Atc).await? {
            Reply::Atc(positions) => Ok(positions),
            _ => Err(AuroraError::UnexpectedReply { expected: "ATC" }),
        }
    }
}

async fn run(
    config: ClientConfig,
    mut requests: mpsc::Receiver<Request>,
    state: watch::Sender<ConnectionState>,
) {
    let mut attempt = 0u32;
    loop {
        let _ = state.send(ConnectionState::Connecting { attempt });
        match TcpStream::connect(config.addr).await {
            Ok(stream) => {
                attempt = 0;
                let _ = state.send(ConnectionState::Connected);
                info!(addr = %config.addr, "connected to Aurora");
                match session(stream, &mut requests, &config).await {
                    SessionEnd::PeerClosed => warn!("Aurora closed the connection"),
                    SessionEnd::Io(error) => warn!(%error, "Aurora connection lost"),
                    SessionEnd::Shutdown => break,
                }
            }
            Err(error) => debug!(%error, attempt, "Aurora connect failed"),
        }
        let _ = state.send(ConnectionState::Disconnected);
        if !wait_for_retry(&mut requests, backoff(&config, attempt)).await {
            break;
        }
        attempt = attempt.saturating_add(1);
    }
    let _ = state.send(ConnectionState::Disconnected);
    info!("Aurora client stopped");
}

/// Rejects requests while offline instead of letting them queue; false means shutdown.
async fn wait_for_retry(requests: &mut mpsc::Receiver<Request>, delay: Duration) -> bool {
    let retry_at = Instant::now() + delay;
    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(retry_at) => return true,
            message = requests.recv() => match message {
                Some(request) => { let _ = request.respond_to.send(Err(AuroraError::Disconnected)); }
                None => return false,
            },
        }
    }
}

async fn session(
    stream: TcpStream,
    requests: &mut mpsc::Receiver<Request>,
    config: &ClientConfig,
) -> SessionEnd {
    if let Err(error) = stream.set_nodelay(true) {
        debug!(%error, "could not disable Nagle on the Aurora socket");
    }
    let (mut reader, mut writer) = stream.into_split();
    let mut queues: Queues = std::array::from_fn(|_| VecDeque::new());
    let mut framer = LineFramer::default();
    let mut outgoing = String::new();
    let mut chunk = [0u8; READ_CHUNK];
    let mut sweep = tokio::time::interval(TIMEOUT_SWEEP);
    sweep.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let end = loop {
        tokio::select! {
            message = requests.recv() => match message {
                Some(request) => {
                    outgoing.clear();
                    request.command.write_into(&mut outgoing);
                    if let Err(error) = writer.write_all(outgoing.as_bytes()).await {
                        let _ = request.respond_to.send(Err(AuroraError::Disconnected));
                        break SessionEnd::Io(error);
                    }
                    queues[request.command.name().index()].push_back(Pending {
                        deadline: Instant::now() + config.request_timeout,
                        respond_to: request.respond_to,
                    });
                }
                None => break SessionEnd::Shutdown,
            },
            read = reader.read(&mut chunk) => match read {
                Ok(0) => break SessionEnd::PeerClosed,
                Ok(n) => {
                    framer.push(&chunk[..n]);
                    framer.drain(|line| match line {
                        Ok(text) => dispatch(&mut queues, text),
                        Err(error) => warn!(%error, "discarding a malformed line"),
                    });
                }
                Err(error) => break SessionEnd::Io(error),
            },
            _ = sweep.tick() => expire(&mut queues, Instant::now(), config.request_timeout),
        }
    };

    fail_all(&mut queues);
    end
}

fn dispatch(queues: &mut Queues, line: &str) {
    let response = match protocol::parse(line) {
        Ok(response) => response,
        Err(error) => {
            warn!(%error, line, "unparseable line from Aurora");
            return;
        }
    };
    match response {
        Response::Conn { station } => {
            complete(queues, CommandName::Conn, Ok(Reply::Conn(station.into())))
        }
        Response::SelTfc { callsign } => complete(
            queues,
            CommandName::SelTfc,
            Ok(Reply::SelTfc(
                (!callsign.is_empty()).then(|| callsign.into()),
            )),
        ),
        Response::FlightPlan(plan) => complete(
            queues,
            CommandName::FlightPlan,
            Ok(Reply::FlightPlan(plan.into())),
        ),
        Response::TrafficPosition(position) => complete(
            queues,
            CommandName::TrafficPosition,
            Ok(Reply::TrafficPosition(position.into())),
        ),
        Response::TrafficList(list) => complete(
            queues,
            CommandName::TrafficList,
            Ok(Reply::TrafficList(list.into())),
        ),
        Response::Atc(list) => complete(queues, CommandName::Atc, Ok(Reply::Atc(list.into()))),
        Response::Refusal(refusal) => match CommandName::from_wire(refusal.command) {
            Some(name) => complete(
                queues,
                name,
                Err(AuroraError::Refused {
                    command: refusal.command.into(),
                    argument: refusal.argument.into(),
                    reason: refusal.reason.into(),
                }),
            ),
            None => warn!(
                command = refusal.command,
                reason = refusal.reason,
                "refusal naming an unknown command"
            ),
        },
        Response::Unknown { command, .. } => debug!(command, "ignoring unsolicited response"),
    }
}

fn complete(queues: &mut Queues, name: CommandName, reply: Result<Reply, AuroraError>) {
    match queues[name.index()].pop_front() {
        Some(pending) => {
            let _ = pending.respond_to.send(reply);
        }
        None => warn!(command = name.as_str(), "response with no pending request"),
    }
}

fn expire(queues: &mut Queues, now: Instant, timeout: Duration) {
    for queue in queues.iter_mut() {
        while queue.front().is_some_and(|pending| pending.deadline <= now) {
            if let Some(pending) = queue.pop_front() {
                let _ = pending.respond_to.send(Err(AuroraError::Timeout(timeout)));
            }
        }
    }
}

fn fail_all(queues: &mut Queues) {
    for queue in queues.iter_mut() {
        for pending in queue.drain(..) {
            let _ = pending.respond_to.send(Err(AuroraError::Disconnected));
        }
    }
}

fn backoff(config: &ClientConfig, attempt: u32) -> Duration {
    let factor = 1u32 << attempt.min(BACKOFF_SHIFT_CAP);
    config
        .backoff_initial
        .saturating_mul(factor)
        .min(config.backoff_max)
}
