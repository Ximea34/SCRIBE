use std::time::Duration;

use thiserror::Error;

use protocol::InvalidCallsign;

pub mod client;
pub mod codec;
pub mod protocol;
pub mod types;

pub use client::{AuroraClient, ClientConfig, ConnectionState, Reply};
pub use protocol::{Command, CommandName, Response};
pub use types::{AtcPosition, FlightPlan, TrafficPosition};

#[derive(Debug, Error)]
pub enum AuroraError {
    #[error("not connected to Aurora")]
    Disconnected,
    #[error("request timed out after {0:?}")]
    Timeout(Duration),
    #[error("Aurora refused {command} {argument:?}: {reason}")]
    Refused {
        command: Box<str>,
        argument: Box<str>,
        reason: Box<str>,
    },
    #[error("expected a {expected} reply")]
    UnexpectedReply { expected: &'static str },
    #[error(transparent)]
    InvalidCallsign(#[from] InvalidCallsign),
    #[error("the Aurora client task has stopped")]
    ClientStopped,
}
