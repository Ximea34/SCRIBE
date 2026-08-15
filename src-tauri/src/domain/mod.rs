/// Milliseconds on a monotonic timeline supplied by the caller, so every domain rule stays
/// pure and testable without a clock.
pub type Millis = u64;

pub mod activations;
pub mod board;
pub mod classifier;
pub mod flight;
pub mod geo;
pub mod ordering;
pub mod store;
pub mod strip;

pub use board::{Board, BoardSnapshot, BoardUpdate, Column, Columns, StripView};
pub use flight::Flight;
pub use store::{ActivationError, DomainConfig, Store};
pub use strip::{ArchiveReason, StripState, TransitionError};
