use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use tracing::warn;

/// Makes each temporary file unique: two saves in flight must not fight over one name.
static WRITE_TICKET: AtomicU64 = AtomicU64::new(0);

/// Keyed on more than the callsign: a callsign recycled the next day must not inherit
/// yesterday's activation, so the departure field and EOBT have to match too (5.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationRecord {
    pub callsign: String,
    pub dep: String,
    pub eobt: String,
    pub activated_at_unix_ms: u64,
}

/// Never fatal: a missing or corrupt file just means no activations to restore.
pub fn load(path: &Path) -> Vec<ActivationRecord> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            warn!(?path, %error, "cannot read saved activations");
            return Vec::new();
        }
    };
    match serde_json::from_str(&text) {
        Ok(records) => records,
        Err(error) => {
            warn!(?path, %error, "discarding unreadable saved activations");
            Vec::new()
        }
    }
}

/// Written through a uniquely named temporary file and renamed into place, so a crash or an
/// overlapping save can never leave a half-written board behind.
pub fn save(path: &Path, records: &[ActivationRecord]) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(records)?;
    let ticket = WRITE_TICKET.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_extension(format!("tmp{ticket}"));

    std::fs::write(&temporary, text)?;
    let renamed = std::fs::rename(&temporary, path);
    if renamed.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    renamed
}
