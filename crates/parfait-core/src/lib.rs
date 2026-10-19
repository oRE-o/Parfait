//! UI-independent raster document, brush engine and bounded undo history.
mod brush;
mod document;
mod history;
mod layer_mask;
mod layer_ops;
mod layer_settings;
mod raster_brush;
mod raster_conversion;
mod smudge;
mod wash;
pub use brush::{Brush, BrushOperation, BrushTip, GapMode, OpacityMode, Point, Stroke};
pub use document::{Document, Layer, LayerId, PixelRect, composite_over};
pub use history::{History, HistoryRow};
pub use layer_mask::{LayerMask, MaskCoverage};
pub use layer_ops::LayerCombine;
pub use raster_conversion::RasterConversion;

mod pressure;
pub use pressure::PressureCalibration;

mod blend;
pub use blend::BlendMode;

mod response_curve;
pub use response_curve::{
    MAX_CURVE_POINTS, ResponseCurve, automatic_curve_controls, reconcile_curve_controls,
    sample_bezier_response, sample_response,
};
