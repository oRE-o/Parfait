//! The v10 byte layout is unchanged for drawings without native content.
use parfait_core::{Document, Layer, MaskCoverage};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
pub(crate) struct LegacyDocument {
    width: u32,
    height: u32,
    name: String,
    layers: Vec<Layer>,
    active: usize,
    revision: u64,
    next_layer_id: u64,
    selection: Option<MaskCoverage>,
}
impl From<LegacyDocument> for Document {
    fn from(d: LegacyDocument) -> Self {
        Self {
            width: d.width,
            height: d.height,
            name: d.name,
            layers: d.layers,
            active: d.active,
            revision: d.revision,
            next_layer_id: d.next_layer_id,
            selection: d.selection,
            contents: Vec::new(),
        }
    }
}
#[derive(Serialize)]
pub(crate) struct Borrowed<'a> {
    width: u32,
    height: u32,
    name: &'a str,
    layers: &'a [Layer],
    active: usize,
    revision: u64,
    next_layer_id: u64,
    selection: &'a Option<MaskCoverage>,
}
impl<'a> From<&'a Document> for Borrowed<'a> {
    fn from(d: &'a Document) -> Self {
        Self {
            width: d.width,
            height: d.height,
            name: &d.name,
            layers: &d.layers,
            active: d.active,
            revision: d.revision,
            next_layer_id: d.next_layer_id,
            selection: &d.selection,
        }
    }
}
