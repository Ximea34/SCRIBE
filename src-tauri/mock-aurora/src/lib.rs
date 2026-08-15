use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

mod traffic;

pub use traffic::Traffic;

/// Deliberate misbehaviours, each reproducing something section 4 says real Aurora does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Quirk {
    /// Answer `@ERR` for this command name, e.g. `"FP"`.
    ErrOn(String),
    /// Never answer this command, so the client's timeout path runs.
    SilentOn(String),
    /// Close the socket without answering this command.
    CloseOn(String),
    /// Deliver every response in two TCP writes.
    SplitWrites,
    /// Terminate lines with a bare `\n` instead of `\r\n`.
    BareNewlines,
    /// Emit one over-long line before the first real answer.
    OversizedLine,
    /// Hold every answer back, so several requests are genuinely in flight at once.
    DelayReplies(Duration),
}

#[derive(Debug, Default, Clone)]
pub struct Script {
    pub station: String,
    pub selected: Option<String>,
    pub traffics: Vec<Traffic>,
    pub quirks: Vec<Quirk>,
}

impl Script {
    pub fn new(station: &str) -> Self {
        Self {
            station: station.to_owned(),
            ..Self::default()
        }
    }

    pub fn with_traffics(mut self, traffics: Vec<Traffic>) -> Self {
        self.traffics = traffics;
        self
    }

    pub fn with_quirk(mut self, quirk: Quirk) -> Self {
        self.quirks.push(quirk);
        self
    }

    fn find(&self, callsign: &str) -> Option<&Traffic> {
        self.traffics.iter().find(|t| t.callsign == callsign)
    }
}

/// What the client actually asked for, so polling budgets can be measured rather than assumed.
#[derive(Debug, Default)]
pub struct Stats {
    pub requests: AtomicU64,
    pub positions: AtomicU64,
    pub flight_plans: AtomicU64,
    pub traffic_lists: AtomicU64,
    positions_by_callsign: Mutex<HashMap<String, u64>>,
}

impl Stats {
    fn record(&self, command: &str, argument: &str) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        match command {
            "TRPOS" => {
                self.positions.fetch_add(1, Ordering::Relaxed);
                if let Ok(mut by_callsign) = self.positions_by_callsign.lock() {
                    *by_callsign.entry(argument.to_owned()).or_default() += 1;
                }
            }
            "FP" => {
                self.flight_plans.fetch_add(1, Ordering::Relaxed);
            }
            "TR" => {
                self.traffic_lists.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    /// Fewest position requests any single callsign received; the full-sweep worst case.
    pub fn least_polled(&self) -> u64 {
        self.positions_by_callsign
            .lock()
            .map(|by_callsign| by_callsign.values().copied().min().unwrap_or(0))
            .unwrap_or(0)
    }

    pub fn distinct_callsigns_polled(&self) -> usize {
        self.positions_by_callsign
            .lock()
            .map_or(0, |by_callsign| by_callsign.len())
    }

    /// Discards the warm-up so a measurement covers steady state, not the cold-start burst.
    pub fn reset(&self) {
        self.requests.store(0, Ordering::Relaxed);
        self.positions.store(0, Ordering::Relaxed);
        self.flight_plans.store(0, Ordering::Relaxed);
        self.traffic_lists.store(0, Ordering::Relaxed);
        if let Ok(mut by_callsign) = self.positions_by_callsign.lock() {
            by_callsign.clear();
        }
    }
}

pub struct MockAurora {
    addr: SocketAddr,
    script: Arc<Mutex<Script>>,
    stats: Arc<Stats>,
    join: JoinHandle<()>,
}

impl MockAurora {
    /// Binds an ephemeral loopback port so tests never collide.
    pub async fn start(script: Script) -> std::io::Result<Self> {
        Self::bind(
            SocketAddr::from(([127, 0, 0, 1], 0)),
            Arc::new(Mutex::new(script)),
        )
        .await
    }

    pub async fn bind(addr: SocketAddr, script: Arc<Mutex<Script>>) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        let stats = Arc::new(Stats::default());
        let join = tokio::spawn(accept_loop(
            listener,
            Arc::clone(&script),
            Arc::clone(&stats),
        ));
        Ok(Self {
            addr,
            script,
            stats,
            join,
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn script(&self) -> Arc<Mutex<Script>> {
        Arc::clone(&self.script)
    }

    pub fn stats(&self) -> Arc<Stats> {
        Arc::clone(&self.stats)
    }
}

impl Drop for MockAurora {
    fn drop(&mut self) {
        self.join.abort();
    }
}

async fn accept_loop(listener: TcpListener, script: Arc<Mutex<Script>>, stats: Arc<Stats>) {
    while let Ok((stream, _)) = listener.accept().await {
        let script = Arc::clone(&script);
        let stats = Arc::clone(&stats);
        tokio::spawn(async move {
            let _ = serve(stream, script, stats).await;
        });
    }
}

async fn serve(
    stream: TcpStream,
    script: Arc<Mutex<Script>>,
    stats: Arc<Stats>,
) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let mut oversized_pending = true;

    while let Some(line) = lines.next_line().await? {
        let line = line.trim_end_matches('\r');
        let body = line.strip_prefix('#').unwrap_or(line);
        let (command, argument) = body.split_once(';').unwrap_or((body, ""));
        stats.record(command, argument);

        let (replies, close, quirks) = {
            let Ok(script) = script.lock() else {
                return Ok(());
            };
            let (replies, close) = respond(line, &script);
            (replies, close, script.quirks.clone())
        };

        if oversized_pending && quirks.contains(&Quirk::OversizedLine) {
            oversized_pending = false;
            let flood = format!("#TR;{}", "X".repeat(32 * 1024));
            write_line(&mut writer, &flood, &quirks).await?;
        }
        if let Some(delay) = quirks.iter().find_map(|quirk| match quirk {
            Quirk::DelayReplies(delay) => Some(*delay),
            _ => None,
        }) {
            tokio::time::sleep(delay).await;
        }
        for reply in replies {
            write_line(&mut writer, &reply, &quirks).await?;
        }
        if close {
            return Ok(());
        }
    }
    Ok(())
}

fn respond(line: &str, script: &Script) -> (Vec<String>, bool) {
    let body = line.strip_prefix('#').unwrap_or(line);
    let (command, argument) = body.split_once(';').unwrap_or((body, ""));

    if script.quirks.contains(&Quirk::CloseOn(command.to_owned())) {
        return (Vec::new(), true);
    }
    if script.quirks.contains(&Quirk::SilentOn(command.to_owned())) {
        return (Vec::new(), false);
    }
    if script.quirks.contains(&Quirk::ErrOn(command.to_owned())) {
        return (vec![refusal(command, argument, "refused by mock")], false);
    }

    match command {
        "CONN" => (vec![format!("#CONN;{}", script.station)], false),
        "SELTFC" => (
            vec![format!(
                "#SELTFC;{}",
                script.selected.as_deref().unwrap_or_default()
            )],
            false,
        ),
        "TR" => {
            let callsigns: Vec<&str> = script
                .traffics
                .iter()
                .map(|t| t.callsign.as_str())
                .collect();
            (vec![format!("#TR;{}", callsigns.join(";"))], false)
        }
        "ATC" => (
            vec!["#ATC;LFLL_APP:120.500;LFLL_TWR:118.100".to_owned()],
            false,
        ),
        "BAY" => (vec!["@BAY;No data in bay".to_owned()], false),
        "FP" | "TRPOS" => {
            // 4.6.1: asking for the selection's position with nothing selected kills the socket.
            if command == "TRPOS" && argument == "%SELTFC%" && script.selected.is_none() {
                return (Vec::new(), true);
            }
            let callsign = match argument {
                "%SELTFC%" => script.selected.as_deref().unwrap_or_default(),
                other => other,
            };
            match script.find(callsign) {
                Some(traffic) if command == "FP" => (vec![traffic.flight_plan_line()], false),
                Some(traffic) => (vec![traffic.position_line()], false),
                None => (vec![refusal(command, argument, "unknown callsign")], false),
            }
        }
        _ => (vec![refusal(command, argument, "unknown command")], false),
    }
}

fn refusal(command: &str, argument: &str, reason: &str) -> String {
    format!("@ERR;#{command};{argument};{reason}")
}

async fn write_line(
    writer: &mut OwnedWriteHalf,
    body: &str,
    quirks: &[Quirk],
) -> std::io::Result<()> {
    let terminator = if quirks.contains(&Quirk::BareNewlines) {
        "\n"
    } else {
        "\r\n"
    };
    let full = format!("{body}{terminator}");
    let bytes = full.as_bytes();

    if quirks.contains(&Quirk::SplitWrites) && bytes.len() > 3 {
        let mid = bytes.len() / 2;
        writer.write_all(&bytes[..mid]).await?;
        tokio::time::sleep(Duration::from_millis(2)).await;
        writer.write_all(&bytes[mid..]).await?;
    } else {
        writer.write_all(bytes).await?;
    }
    Ok(())
}
