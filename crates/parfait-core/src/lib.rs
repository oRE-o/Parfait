//! UI-independent raster document, brush engine and bounded undo history.
mod brush;
mod document;
mod history;
pub use brush::{Brush, BrushTip, GapMode, Point, Stroke};
pub use document::{Document, Layer, PixelRect, composite_over};
pub use history::History;

mod pressure;
pub use pressure::PressureCalibration;

mod blend;
pub use blend::BlendMode;

mod response_curve;
pub use response_curve::{
    MAX_CURVE_POINTS, ResponseCurve, automatic_curve_controls, reconcile_curve_controls,
    sample_bezier_response, sample_response,
};
