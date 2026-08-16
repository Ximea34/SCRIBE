use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

use super::naming::{self, NameError};
use super::{StripTemplate, SCHEMA_VERSION};

pub const DIRECTORY: &str = "strips";

/// Unique per write, so two saves in flight cannot delete each other's temporary file.
static WRITE_TICKET: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind", content = "detail")]
pub enum StorageError {
    #[error("{0}")]
    Name(NameError),
    #[error("{0}")]
    Directory(String),
    #[error("{0}")]
    File(String),
    #[error("{0}")]
    Schema(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TemplateListing {
    pub file_name: String,
    pub name: String,
    /// An unparseable file is listed, flagged and still deletable — never silently hidden.
    pub valid: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "outcome", content = "fileName")]
pub enum SaveOutcome {
    Saved(String),
    /// The target exists and is not the file being replaced; overwriting would destroy a
    /// colleague's template on a shared event machine.
    NeedsConfirmation(String),
}

pub fn directory(app_data: &Path) -> PathBuf {
    app_data.join(DIRECTORY)
}

/// A missing directory simply means no templates yet.
pub fn list(directory: &Path) -> Result<Vec<TemplateListing>, StorageError> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let entries = std::fs::read_dir(directory).map_err(|error| {
        StorageError::Directory(format!("cannot read {}: {error}", directory.display()))
    })?;

    let mut listings: Vec<TemplateListing> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .map(|entry| {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            match read(&entry.path(), &file_name) {
                Ok(template) => TemplateListing {
                    file_name,
                    name: template.name,
                    valid: true,
                    error: None,
                },
                Err(error) => TemplateListing {
                    name: file_name.clone(),
                    file_name,
                    valid: false,
                    error: Some(error.to_string()),
                },
            }
        })
        .collect();

    listings.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
    Ok(listings)
}

pub fn load(directory: &Path, file_name: &str) -> Result<StripTemplate, StorageError> {
    let path = resolve(directory, file_name)?;
    read(&path, file_name)
}

/// Validates the name and writes atomically. It never deletes anything: saving under a new name
/// creates a second template rather than renaming the first, so a controller can build one strip
/// from another without losing it. Removing the old one is the explorer's job, and deliberate.
pub fn save(
    directory: &Path,
    template: &StripTemplate,
    bound: Option<&str>,
    overwrite: bool,
) -> Result<SaveOutcome, StorageError> {
    let parsed = naming::parse(&template.name).map_err(StorageError::Name)?;
    let file_name = naming::file_name(&parsed);
    let path = resolve(directory, &file_name)?;

    // Saving onto the file the editor is already bound to needs no confirmation.
    let is_bound_file = bound == Some(file_name.as_str());
    if path.exists() && !is_bound_file && !overwrite {
        return Ok(SaveOutcome::NeedsConfirmation(file_name));
    }

    let stored = StripTemplate {
        schema_version: SCHEMA_VERSION,
        name: parsed.normalised,
        icao: parsed.icao,
        position: parsed.position,
        kind: parsed.kind,
        size: template.size,
        fields: template.fields.clone(),
        elements: template.elements.clone(),
    };
    let json = serde_json::to_string_pretty(&stored)
        .map_err(|error| StorageError::File(format!("cannot serialise the template: {error}")))?;

    write_atomic(&path, &json)
        .map_err(|error| StorageError::File(format!("cannot write {file_name}: {error}")))?;

    Ok(SaveOutcome::Saved(file_name))
}

pub fn delete(directory: &Path, file_name: &str) -> Result<(), StorageError> {
    let path = resolve(directory, file_name)?;
    std::fs::remove_file(&path)
        .map_err(|error| StorageError::File(format!("cannot delete {file_name}: {error}")))
}

/// Reads the version before the body, so a newer file fails with a clear message instead of
/// silently mis-parsing.
fn read(path: &Path, file_name: &str) -> Result<StripTemplate, StorageError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Probe {
        schema_version: u32,
    }

    let text = std::fs::read_to_string(path)
        .map_err(|error| StorageError::File(format!("cannot read {file_name}: {error}")))?;

    let probe: Probe = serde_json::from_str(&text)
        .map_err(|error| StorageError::File(format!("{file_name} is not a template: {error}")))?;
    if probe.schema_version != SCHEMA_VERSION {
        return Err(StorageError::Schema(format!(
            "{file_name} uses schema version {}, this build understands {SCHEMA_VERSION}",
            probe.schema_version
        )));
    }

    serde_json::from_str(&text)
        .map_err(|error| StorageError::File(format!("{file_name} is malformed: {error}")))
}

fn resolve(directory: &Path, file_name: &str) -> Result<PathBuf, StorageError> {
    if file_name.is_empty() || file_name.contains(['/', '\\']) || file_name.contains("..") {
        return Err(StorageError::File(format!(
            "{file_name:?} is not a safe file name"
        )));
    }
    Ok(directory.join(file_name))
}

fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let ticket = WRITE_TICKET.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_extension(format!("tmp{ticket}"));

    {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
    }

    let renamed = std::fs::rename(&temporary, path);
    if renamed.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    renamed
}
