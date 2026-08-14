use thiserror::Error;

use crate::aurora::AuroraError;

/// Top-level error type; every subsystem error converts into one of these variants.
#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Aurora(#[from] AuroraError),
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
}

pub type AppResult<T> = Result<T, AppError>;
