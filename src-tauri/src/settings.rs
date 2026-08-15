use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::aurora::client::{ClientConfig, DEFAULT_PORT};
use crate::aurora::scheduler::SchedulerConfig;
use crate::domain::DomainConfig;

pub const ENV_AIRPORTS_FILE: &str = "SCRIBE_AIRPORTS_FILE";
pub const ENV_ICAO: &str = "SCRIBE_ICAO";
pub const ENV_AURORA_ADDR: &str = "SCRIBE_AURORA_ADDR";

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("cannot read {path:?}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot write {path:?}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path:?} is not valid settings JSON: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

/// Every cadence, budget and threshold in the app lives here; nothing tunable is hard-coded
/// anywhere else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub aurora_host: IpAddr,
    pub aurora_port: u16,
    pub airports_file: Option<PathBuf>,
    pub selected_icao: Option<String>,
    pub ring_radius_nm: f64,
    pub connection: Connection,
    pub polling: Polling,
    pub removal: Removal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Connection {
    pub request_timeout_ms: u64,
    pub backoff_initial_ms: u64,
    pub backoff_max_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Polling {
    pub budget_requests_per_second: u32,
    pub traffic_list_interval_ms: u64,
    pub station_interval_ms: u64,
    pub flight_plan_ttl_ms: u64,
    pub flight_plan_max_attempts: u32,
    pub flight_plan_retry_backoff_ms: u64,
    pub board_refresh_ms: u64,
    pub near_refresh_ms: u64,
    pub far_refresh_ms: u64,
    pub emit_interval_ms: u64,
    pub max_position_age_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Removal {
    pub parked_ground_speed_kt: u16,
    pub parked_hold_ms: u64,
    pub radar_dropout_grace_ms: u64,
    pub archive_ttl_ms: u64,
    pub activation_max_age_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            aurora_host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            aurora_port: DEFAULT_PORT,
            airports_file: None,
            selected_icao: None,
            ring_radius_nm: 20.0,
            connection: Connection::default(),
            polling: Polling::default(),
            removal: Removal::default(),
        }
    }
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            request_timeout_ms: 2_000,
            backoff_initial_ms: 250,
            backoff_max_ms: 8_000,
        }
    }
}

impl Default for Polling {
    fn default() -> Self {
        Self {
            budget_requests_per_second: 150,
            traffic_list_interval_ms: 1_000,
            station_interval_ms: 30_000,
            flight_plan_ttl_ms: 60_000,
            flight_plan_max_attempts: 5,
            flight_plan_retry_backoff_ms: 2_000,
            board_refresh_ms: 1_000,
            near_refresh_ms: 2_000,
            far_refresh_ms: 4_000,
            emit_interval_ms: 100,
            max_position_age_ms: 15_000,
        }
    }
}

impl Default for Removal {
    fn default() -> Self {
        Self {
            parked_ground_speed_kt: 3,
            parked_hold_ms: 15_000,
            radar_dropout_grace_ms: 30_000,
            archive_ttl_ms: 120_000,
            activation_max_age_ms: 6 * 60 * 60 * 1_000,
        }
    }
}

impl Settings {
    /// A missing file is not an error: it means first run, so defaults apply.
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|source| SettingsError::Parse {
                path: path.to_path_buf(),
                source,
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(SettingsError::Read {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        let write = |source| SettingsError::Write {
            path: path.to_path_buf(),
            source,
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(write)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|source| SettingsError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        let temporary = path.with_extension("tmp");
        std::fs::write(&temporary, text).map_err(write)?;
        std::fs::rename(&temporary, path).map_err(write)
    }

    /// Developer escape hatch until the OPTIONS tab exists; takes the lookup as an argument
    /// so it can be tested without touching the process environment.
    pub fn apply_overrides(&mut self, lookup: impl Fn(&str) -> Option<String>) {
        if let Some(file) = lookup(ENV_AIRPORTS_FILE) {
            self.airports_file = Some(PathBuf::from(file));
        }
        if let Some(icao) = lookup(ENV_ICAO) {
            self.selected_icao = Some(icao.to_ascii_uppercase());
        }
        if let Some(addr) = lookup(ENV_AURORA_ADDR) {
            match addr.parse::<SocketAddr>() {
                Ok(addr) => {
                    self.aurora_host = addr.ip();
                    self.aurora_port = addr.port();
                }
                Err(error) => {
                    tracing::warn!(%addr, %error, "ignoring an unparseable {ENV_AURORA_ADDR}");
                }
            }
        }
    }

    pub fn apply_env_overrides(&mut self) {
        self.apply_overrides(|name| std::env::var(name).ok());
    }

    pub fn aurora_addr(&self) -> SocketAddr {
        SocketAddr::new(self.aurora_host, self.aurora_port)
    }

    pub fn client_config(&self) -> ClientConfig {
        ClientConfig {
            addr: self.aurora_addr(),
            request_timeout: Duration::from_millis(self.connection.request_timeout_ms),
            backoff_initial: Duration::from_millis(self.connection.backoff_initial_ms),
            backoff_max: Duration::from_millis(self.connection.backoff_max_ms),
        }
    }

    pub fn scheduler_config(&self) -> SchedulerConfig {
        SchedulerConfig {
            budget_per_second: self.polling.budget_requests_per_second,
            traffic_list_interval: self.polling.traffic_list_interval_ms,
            station_interval: self.polling.station_interval_ms,
            board_refresh: self.polling.board_refresh_ms,
            near_refresh: self.polling.near_refresh_ms,
            far_refresh: self.polling.far_refresh_ms,
            in_flight_grace: self.connection.request_timeout_ms.saturating_mul(2),
        }
    }

    pub fn domain_config(&self) -> DomainConfig {
        DomainConfig {
            ring_radius_nm: self.ring_radius_nm,
            max_position_age: self.polling.max_position_age_ms,
            radar_dropout_grace: self.removal.radar_dropout_grace_ms,
            archive_ttl: self.removal.archive_ttl_ms,
            parked_ground_speed_kt: self.removal.parked_ground_speed_kt,
            parked_hold: self.removal.parked_hold_ms,
            flight_plan_max_attempts: self.polling.flight_plan_max_attempts,
            flight_plan_retry_backoff: self.polling.flight_plan_retry_backoff_ms,
            activation_max_age: self.removal.activation_max_age_ms,
        }
    }
}
