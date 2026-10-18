//! UI-independent raster document, brush engine and bounded undo history.
mod brush;
mod document;
mod history;
pub use brush::{Brush, BrushTip, Point, Stroke};
pub use document::{Document, Layer, PixelRect, composite_over};
pub use history::History;

mod pressure;
pub use pressure::PressureCalibration;
