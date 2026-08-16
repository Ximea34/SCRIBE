use std::path::Path;

use base64::Engine;
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

/// Templates carry their logo inline, so the cap keeps a shared JSON file a sane size.
pub const MAX_BYTES: usize = 512 * 1024;

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind", content = "detail")]
pub enum LogoError {
    #[error("{0}")]
    Read(String),
    #[error("the image is {bytes} bytes; the limit is {limit}")]
    TooLarge { bytes: u32, limit: u32 },
    #[error("only PNG and JPEG images are supported")]
    Unsupported,
    #[error("the image dimensions could not be read")]
    Dimensions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportedLogo {
    pub mime: String,
    pub data: String,
    pub width_px: u32,
    pub height_px: u32,
}

/// Reads and validates in Rust so the front end never needs a filesystem capability.
pub fn import(path: &Path) -> Result<ImportedLogo, LogoError> {
    let bytes = std::fs::read(path).map_err(|error| LogoError::Read(error.to_string()))?;
    if bytes.len() > MAX_BYTES {
        return Err(LogoError::TooLarge {
            bytes: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
            limit: u32::try_from(MAX_BYTES).unwrap_or(u32::MAX),
        });
    }

    // The magic bytes decide the type, not the extension.
    let (mime, dimensions) = if bytes.starts_with(&PNG_SIGNATURE) {
        ("image/png", png_dimensions(&bytes))
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        ("image/jpeg", jpeg_dimensions(&bytes))
    } else {
        return Err(LogoError::Unsupported);
    };

    let (width_px, height_px) = dimensions.ok_or(LogoError::Dimensions)?;
    Ok(ImportedLogo {
        mime: mime.to_owned(),
        data: base64::engine::general_purpose::STANDARD.encode(&bytes),
        width_px,
        height_px,
    })
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let width = bytes.get(16..20)?;
    let height = bytes.get(20..24)?;
    Some((be_u32(width)?, be_u32(height)?))
}

/// Walks the segment chain to the first start-of-frame marker, which carries the dimensions.
fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut cursor = 2usize;
    loop {
        if *bytes.get(cursor)? != 0xFF {
            return None;
        }
        let marker = *bytes.get(cursor + 1)?;
        cursor += 2;

        // Standalone markers carry no payload.
        if matches!(marker, 0xD8 | 0xD9) || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        let length = be_u16(bytes.get(cursor..cursor + 2)?)? as usize;
        if length < 2 {
            return None;
        }

        let is_start_of_frame =
            matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
        if is_start_of_frame {
            let height = be_u16(bytes.get(cursor + 3..cursor + 5)?)?;
            let width = be_u16(bytes.get(cursor + 5..cursor + 7)?)?;
            return Some((u32::from(width), u32::from(height)));
        }
        cursor += length;
    }
}

fn be_u32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.try_into().ok()?))
}

fn be_u16(bytes: &[u8]) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.try_into().ok()?))
}
