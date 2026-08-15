use tauri::State;

use crate::domain::BoardSnapshot;

use super::{FlightDetail, Ipc, IpcError};

/// The whole board, for a front end that has just mounted.
#[tauri::command]
#[specta::specta]
pub async fn board_snapshot(ipc: State<'_, Ipc>) -> Result<BoardSnapshot, IpcError> {
    ipc.engine()?.snapshot().await
}

/// Moves an awake departure into ACTIVÉS; rejected for anything else.
#[tauri::command]
#[specta::specta]
pub async fn activate_flight(ipc: State<'_, Ipc>, callsign: String) -> Result<(), IpcError> {
    ipc.engine()?.activate(callsign).await
}

#[tauri::command]
#[specta::specta]
pub async fn flight_detail(
    ipc: State<'_, Ipc>,
    callsign: String,
) -> Result<Option<FlightDetail>, IpcError> {
    ipc.engine()?.detail(callsign).await
}
