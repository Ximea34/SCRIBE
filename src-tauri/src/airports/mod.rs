use std::collections::HashMap;
use std::path::{Path, PathBuf};

use thiserror::Error;
use tracing::{debug, warn};

use crate::domain::geo::LatLon;

pub mod coords;
pub mod parser;

pub use parser::{parse_airports, AirportLineError, IssueKind, LineIssue};

#[derive(Debug, Clone, PartialEq)]
pub struct Airport {
    pub icao: Box<str>,
    pub name: Box<str>,
    pub lat: f64,
    pub lon: f64,
    pub elevation_ft: i32,
}

impl Airport {
    pub fn centre(&self) -> LatLon {
        LatLon::new(self.lat, self.lon)
    }
}

#[derive(Debug, Clone, Default)]
pub struct AirportRegistry {
    by_icao: HashMap<Box<str>, Airport>,
}

impl AirportRegistry {
    pub fn get(&self, icao: &str) -> Option<&Airport> {
        self.by_icao.get(icao)
    }

    pub fn len(&self) -> usize {
        self.by_icao.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_icao.is_empty()
    }

    /// Last definition wins; returns the entry it replaced, if any.
    pub fn insert(&mut self, airport: Airport) -> Option<Airport> {
        self.by_icao.insert(airport.icao.clone(), airport)
    }

    pub fn icaos(&self) -> impl Iterator<Item = &str> {
        self.by_icao.keys().map(|icao| &**icao)
    }
}

#[derive(Debug, Error)]
pub enum AirportError {
    #[error("no airport configuration file is set; use the SCRIBE_AIRPORTS_FILE override")]
    NotConfigured,
    #[error("cannot read the airport configuration {path:?}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("the airport configuration {path:?} defines no usable airport")]
    Empty { path: PathBuf },
    #[error("airport {icao} is not defined in {path:?}")]
    SelectedNotFound { icao: String, path: PathBuf },
}

pub fn load(path: &Path) -> Result<(AirportRegistry, Vec<LineIssue>), AirportError> {
    let text = std::fs::read_to_string(path).map_err(|source| AirportError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(parse_airports(&text))
}

/// Loads the file, reports every skipped line, and fails hard if the selected ICAO is absent (5.5).
pub fn load_selected(path: &Path, icao: &str) -> Result<Airport, AirportError> {
    let (registry, issues) = load(path)?;
    report(path, &issues);

    if registry.is_empty() {
        return Err(AirportError::Empty {
            path: path.to_path_buf(),
        });
    }
    registry
        .get(&icao.to_ascii_uppercase())
        .cloned()
        .ok_or_else(|| AirportError::SelectedNotFound {
            icao: icao.to_ascii_uppercase(),
            path: path.to_path_buf(),
        })
}

fn report(path: &Path, issues: &[LineIssue]) {
    for issue in issues {
        match &issue.kind {
            IssueKind::Malformed(error) => {
                warn!(?path, line = issue.line, %error, "skipping malformed airport line");
            }
            IssueKind::DuplicateIcao(icao) => {
                warn!(?path, line = issue.line, %icao, "duplicate airport, last definition wins");
            }
            IssueKind::ExtraColumns(count) => {
                debug!(
                    ?path,
                    line = issue.line,
                    count,
                    "ignoring extra airport columns"
                );
            }
        }
    }
}
