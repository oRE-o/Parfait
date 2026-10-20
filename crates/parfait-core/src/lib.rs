//! UI-independent raster document, brush engine and bounded undo history.
mod art_digest;
mod edit_state;
pub use art_digest::ArtDigestV1;
pub use edit_state::EditStateId;
mod brush;
mod document;
mod history;
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
