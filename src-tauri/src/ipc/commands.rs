use std::path::PathBuf;

use tauri::State;

use crate::domain::BoardSnapshot;
use crate::templates::logo::{ImportedLogo, LogoError};
use crate::templates::storage::{SaveOutcome, StorageError, TemplateListing};
use crate::templates::{catalogue, storage, CatalogueEntry, StripTemplate};

use super::{FlightDetail, Ipc, IpcError, Templates};

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

/// Static and compiled in: the editor never asks Aurora for it.
#[tauri::command]
#[specta::specta]
pub async fn get_field_catalogue() -> Vec<CatalogueEntry> {
    catalogue::catalogue()
}

#[tauri::command]
#[specta::specta]
pub async fn list_templates(
    templates: State<'_, Templates>,
) -> Result<Vec<TemplateListing>, StorageError> {
    storage::list(&directory(&templates))
}

#[tauri::command]
#[specta::specta]
pub async fn load_template(
    templates: State<'_, Templates>,
    file_name: String,
) -> Result<StripTemplate, StorageError> {
    storage::load(&directory(&templates), &file_name)
}

/// `bound` is the file the editor currently owns; it only decides whether an existing target
/// needs confirming. Saving never deletes, so a new name always yields a new template.
#[tauri::command]
#[specta::specta]
pub async fn save_template(
    templates: State<'_, Templates>,
    template: StripTemplate,
    bound: Option<String>,
    overwrite: bool,
) -> Result<SaveOutcome, StorageError> {
    storage::save(
        &directory(&templates),
        &template,
        bound.as_deref(),
        overwrite,
    )
}

#[tauri::command]
#[specta::specta]
pub async fn delete_template(
    templates: State<'_, Templates>,
    file_name: String,
) -> Result<(), StorageError> {
    storage::delete(&directory(&templates), &file_name)
}

/// The picker runs in the front end; reading and validating stays here so the webview never
/// needs a filesystem capability.
#[tauri::command]
#[specta::specta]
pub async fn import_logo(path: String) -> Result<ImportedLogo, LogoError> {
    crate::templates::logo::import(std::path::Path::new(&path))
}

fn directory(templates: &State<'_, Templates>) -> PathBuf {
    templates.0.clone()
}
