//! UI-independent raster document, brush engine and bounded undo history.
mod art_digest;
mod edit_state;
pub use art_digest::ArtDigestV1;
pub use edit_state::EditStateId;
mod brush;
mod content;
pub use content::deserialize_contents;
mod document;
pub use content::{
    Affine, ContentBox, ContentNode, LayerContent, MAX_CONTENT_FONT_BYTES,
    MAX_CONTENT_PATHS, MAX_CONTENT_POINTS, TextContent, VectorContent,
    VectorPath,
};
mod history;
mod image_edit;
mod image_import;
pub use image_edit::{PixelClipboard, PixelEdit};
mod tools;
pub use tools::SampleSource;
mod tone;
pub use tone::{
    GradientStop, ToneAdjustment, ToneLevels, automatic_tone_range,
    preserve_luminance,
};
mod transform;
pub use transform::{RasterTransform, WarpKind, WarpPin};
mod alignment;
pub use alignment::Align;
mod canvas_edit;
mod color_assist;
mod local_shading;
pub use canvas_edit::CanvasEdit;
pub use color_assist::RegionColors;
pub use local_shading::LocalShading;
mod layer_folders;
mod layer_mask;
mod layer_ops;
mod layer_settings;
mod raster_brush;
mod raster_conversion;
mod selection;
mod smudge;
mod wash;
pub use brush::{
    Brush, BrushOperation, BrushTip, GapMode, OpacityMode, Point, Stroke,
};
pub use document::{Document, Layer, LayerId, PixelRect, composite_over};
pub use history::{History, HistoryRow};
pub use layer_folders::LayerKind;
pub use layer_mask::{LayerMask, MaskCoverage};
pub use layer_ops::LayerCombine;
pub use raster_conversion::RasterConversion;
pub use selection::SelectionMode;

mod pressure;
mod procedural;
pub use pressure::PressureCalibration;

mod blend;
pub use blend::BlendMode;

mod response_curve;
pub use response_curve::{
    MAX_CURVE_POINTS, ResponseCurve, automatic_curve_controls,
    reconcile_curve_controls, sample_bezier_response, sample_response,
};

mod memory;
pub use memory::{BufferCensus, deserialize_pixel_buffer};
