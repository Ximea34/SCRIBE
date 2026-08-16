use serde::{Deserialize, Serialize};
use specta::Type;

pub mod catalogue;
pub mod logo;
pub mod naming;
pub mod storage;

pub use catalogue::{CatalogueEntry, FieldSource};
pub use naming::{NameError, TemplateName};
pub use storage::{SaveOutcome, StorageError, TemplateListing};

/// Bumped only when the on-disk shape changes incompatibly; an unknown version refuses to load.
pub const SCHEMA_VERSION: u32 = 1;

/// Everything geometric is millimetres from the strip's top-left corner, font sizes are points.
/// Pixels never enter the document — they belong to the canvas render layer alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StripTemplate {
    pub schema_version: u32,
    pub name: String,
    pub icao: String,
    pub position: String,
    pub kind: String,
    pub size: StripSize,
    pub fields: Vec<TemplateField>,
    pub elements: Vec<DesignElement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StripSize {
    pub length_mm: f64,
    pub width_mm: f64,
}

/// One entry per catalogue key holding N placements: the font size belongs to the field and
/// applies to all of them, which is why this is not flattened to a list of placements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TemplateField {
    pub key: String,
    pub font_size_pt: f64,
    pub placements: Vec<Placement>,
}

/// `xMm` / `yMm` is the top-left of the text box, so changing a font size grows the text
/// down-and-right and never moves its origin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub id: String,
    pub x_mm: f64,
    pub y_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DesignElement {
    Line(LineElement),
    Frame(FrameElement),
    Text(TextElement),
    Image(ImageElement),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LineElement {
    pub id: String,
    pub x_mm: f64,
    pub y_mm: f64,
    pub length_mm: f64,
    pub thickness_mm: f64,
    pub orientation: Orientation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FrameElement {
    pub id: String,
    pub x_mm: f64,
    pub y_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
    pub thickness_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TextElement {
    pub id: String,
    pub x_mm: f64,
    pub y_mm: f64,
    pub content: String,
    pub font_size_pt: f64,
}

/// The image travels as base64 inside the JSON so a template stays portable when it is shared
/// before an event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImageElement {
    pub id: String,
    pub x_mm: f64,
    pub y_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
    pub mime: String,
    pub data: String,
}

impl DesignElement {
    pub fn id(&self) -> &str {
        match self {
            Self::Line(element) => &element.id,
            Self::Frame(element) => &element.id,
            Self::Text(element) => &element.id,
            Self::Image(element) => &element.id,
        }
    }
}
