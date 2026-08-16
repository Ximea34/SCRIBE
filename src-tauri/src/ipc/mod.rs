use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

use crate::domain::store::ActivationError;
use crate::domain::Flight;
use crate::engine::EngineHandle;

pub mod commands;
pub mod events;

/// Present only once the board is running; absent when no airport is configured.
pub struct Ipc(pub Option<EngineHandle>);

/// Where strip templates live. Managed independently of the board, because the editor works
/// with Aurora disconnected and without an airport configured.
pub struct Templates(pub PathBuf);

impl Ipc {
    fn engine(&self) -> Result<&EngineHandle, IpcError> {
        self.0.as_ref().ok_or(IpcError::NotRunning)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind", content = "message")]
pub enum IpcError {
    #[error("the board is not running")]
    NotRunning,
    #[error("no flight {0} is being tracked")]
    UnknownCallsign(String),
    #[error("{0}")]
    Rejected(String),
}

impl From<ActivationError> for IpcError {
    fn from(error: ActivationError) -> Self {
        match error {
            ActivationError::UnknownCallsign(callsign) => Self::UnknownCallsign(callsign),
            ActivationError::Transition(transition) => Self::Rejected(transition.to_string()),
        }
    }
}

/// Everything the activation dialog shows. Deliberately off the high-frequency board stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FlightDetail {
    pub callsign: String,
    pub aircraft: String,
    pub wake: String,
    pub rules: String,
    pub flight_type: String,
    pub dep: String,
    pub arr: String,
    pub alternate: String,
    pub eobt: String,
    pub cruise_level: String,
    pub route: String,
    pub squawk: String,
    pub assumed_by: String,
    pub stand: String,
}

impl From<&Flight> for FlightDetail {
    fn from(flight: &Flight) -> Self {
        let position = flight.position.as_ref();
        Self {
            callsign: flight.callsign.to_string(),
            aircraft: flight.plan.aircraft.to_string(),
            wake: flight.plan.wake.to_string(),
            rules: flight.plan.rules.to_string(),
            flight_type: flight.plan.flight_type.to_string(),
            dep: flight.plan.dep.to_string(),
            arr: flight.plan.arr.to_string(),
            alternate: flight.plan.alternate.to_string(),
            eobt: flight.plan.eobt.to_string(),
            cruise_level: flight.plan.cruise_level.to_string(),
            route: flight.plan.route.to_string(),
            squawk: position.map_or_else(String::new, |p| p.squawk_set.to_string()),
            assumed_by: position.map_or_else(String::new, |p| p.assumed_by.to_string()),
            stand: position.map_or_else(String::new, |p| p.gate.to_string()),
        }
    }
}
