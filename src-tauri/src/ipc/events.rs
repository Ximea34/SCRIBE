use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use tauri_specta::Event;
use tracing::warn;

use crate::domain::BoardUpdate;
use crate::engine::BoardSink;

/// Coalesced board delta. Emitted at most every `emitIntervalMs`, never once per parsed line.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct BoardUpdated(pub BoardUpdate);

pub struct TauriSink(pub AppHandle);

impl BoardSink for TauriSink {
    fn emit(&self, update: BoardUpdate) {
        if let Err(error) = BoardUpdated(update).emit(&self.0) {
            warn!(%error, "cannot deliver a board update");
        }
    }
}
