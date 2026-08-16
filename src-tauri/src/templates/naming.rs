use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

/// Single source for the token lists; more positions arrive as the other tabs land.
pub const POSITIONS: [&str; 2] = ["VIGIE", "IFR"];
pub const KINDS: [&str; 3] = ["DEPARTURE", "ARRIVAL", "TRANSIT"];
pub const SUFFIX: &str = "STRIP";

const ICAO_LENGTH: usize = 4;

/// A template is found at print time by ICAO + position + type, so the three tokens are parsed
/// here at save time and stored explicitly. The file name is never re-parsed later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TemplateName {
    pub normalised: String,
    pub icao: String,
    pub position: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind", content = "detail")]
pub enum NameError {
    #[error("the name is empty")]
    Empty,
    #[error("expected four words: ICAO POSITION TYPE STRIP, found {0}")]
    WordCount(u32),
    #[error("{0:?} is not a four-letter ICAO code")]
    Icao(String),
    #[error("{0:?} is not a known position")]
    Position(String),
    #[error("{0:?} is not a known strip type")]
    Kind(String),
    #[error("the name must end with STRIP, found {0:?}")]
    Suffix(String),
}

/// Upper-cases and collapses whitespace so `  lfll  vigie departure strip ` validates.
pub fn normalise(raw: &str) -> String {
    raw.split_whitespace()
        .map(str::to_ascii_uppercase)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn parse(raw: &str) -> Result<TemplateName, NameError> {
    let normalised = normalise(raw);
    if normalised.is_empty() {
        return Err(NameError::Empty);
    }

    let words: Vec<&str> = normalised.split(' ').collect();
    let [icao, position, kind, suffix] = words.as_slice() else {
        return Err(NameError::WordCount(
            u32::try_from(words.len()).unwrap_or(u32::MAX),
        ));
    };

    if !is_icao(icao) {
        return Err(NameError::Icao((*icao).to_owned()));
    }
    if !POSITIONS.contains(position) {
        return Err(NameError::Position((*position).to_owned()));
    }
    if !KINDS.contains(kind) {
        return Err(NameError::Kind((*kind).to_owned()));
    }
    if *suffix != SUFFIX {
        return Err(NameError::Suffix((*suffix).to_owned()));
    }

    Ok(TemplateName {
        icao: (*icao).to_owned(),
        position: (*position).to_owned(),
        kind: (*kind).to_owned(),
        normalised,
    })
}

/// The validated name is already restricted to A-Z and spaces; the filter is a belt-and-braces
/// guard so nothing can ever reach the filesystem with a separator in it.
pub fn file_name(name: &TemplateName) -> String {
    let stem: String = name
        .normalised
        .chars()
        .map(|c| if c == ' ' { '_' } else { c })
        .filter(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
        .collect();
    format!("{stem}.json")
}

fn is_icao(candidate: &str) -> bool {
    candidate.len() == ICAO_LENGTH && candidate.bytes().all(|b| b.is_ascii_uppercase())
}
