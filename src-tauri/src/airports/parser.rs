use thiserror::Error;

use super::coords::{self, Axis, CoordError};
use super::{Airport, AirportRegistry};

const BOM: char = '\u{feff}';
const COMMENT: char = '#';

#[derive(Debug, Clone, Error, PartialEq)]
pub enum AirportLineError {
    #[error("expected at least 5 fields, found {0}")]
    TooFewFields(usize),
    #[error("{0:?} is not a four-letter ICAO code")]
    BadIcao(String),
    #[error("latitude: {0}")]
    Latitude(CoordError),
    #[error("longitude: {0}")]
    Longitude(CoordError),
    #[error("{0:?} is not an elevation in feet")]
    BadElevation(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum IssueKind {
    Malformed(AirportLineError),
    DuplicateIcao(Box<str>),
    ExtraColumns(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LineIssue {
    /// 1-based, so it matches what an editor shows.
    pub line: usize,
    pub kind: IssueKind,
}

/// Parses the whole file; a bad line is skipped and reported, never fatal (5.5).
pub fn parse_airports(text: &str) -> (AirportRegistry, Vec<LineIssue>) {
    let mut registry = AirportRegistry::default();
    let mut issues = Vec::new();
    let text = text.strip_prefix(BOM).unwrap_or(text);

    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with(COMMENT) {
            continue;
        }
        match parse_line(line) {
            Ok((airport, extra)) => {
                if extra > 0 {
                    issues.push(LineIssue {
                        line: number,
                        kind: IssueKind::ExtraColumns(extra),
                    });
                }
                if let Some(replaced) = registry.insert(airport) {
                    issues.push(LineIssue {
                        line: number,
                        kind: IssueKind::DuplicateIcao(replaced.icao),
                    });
                }
            }
            Err(error) => issues.push(LineIssue {
                line: number,
                kind: IssueKind::Malformed(error),
            }),
        }
    }
    (registry, issues)
}

fn parse_line(line: &str) -> Result<(Airport, usize), AirportLineError> {
    let fields: Vec<&str> = line.split(';').collect();
    let [icao, name, lat, lon, elevation, extra @ ..] = fields.as_slice() else {
        return Err(AirportLineError::TooFewFields(fields.len()));
    };

    let icao = icao.trim().to_ascii_uppercase();
    if !is_icao(&icao) {
        return Err(AirportLineError::BadIcao(icao));
    }
    let lat = coords::parse(lat, Axis::Latitude).map_err(AirportLineError::Latitude)?;
    let lon = coords::parse(lon, Axis::Longitude).map_err(AirportLineError::Longitude)?;
    let elevation = elevation.trim();
    let elevation_ft = elevation
        .parse::<i32>()
        .map_err(|_| AirportLineError::BadElevation(elevation.to_owned()))?;

    let airport = Airport {
        icao: icao.into(),
        name: name.trim().into(),
        lat,
        lon,
        elevation_ft,
    };
    Ok((airport, extra.len()))
}

fn is_icao(candidate: &str) -> bool {
    candidate.len() == 4 && candidate.bytes().all(|b| b.is_ascii_uppercase())
}
