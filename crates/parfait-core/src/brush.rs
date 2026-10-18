use crate::{Document, composite_over};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
}
#[derive(Clone)]
pub enum BrushTip {
    Round,
    Soft,
    Image {
        width: u32,
        height: u32,
        mask: Arc<Vec<u8>>,
    },
}
#[derive(Clone)]
pub struct Brush {
    pub size: f32,
    pub opacity: f32,
    pub color: [u8; 3],
    pub erase: bool,
    pub pressure: bool,
    pub tip: BrushTip,
}
impl Default for Brush {
    fn default() -> Self {
        Self {
            size: 12.,
            opacity: 1.,
            color: [46, 43, 54],
            erase: false,
            pressure: true,
            tip: BrushTip::Round,
        }
    }
}
pub struct Stroke {
    last: Point,
    spacing_left: f32,
    brush: Brush,
}
impl Stroke {
    pub fn begin(doc: &mut Document, brush: Brush, point: Point) -> Self {
        stamp(doc, &brush, point);
        Self {
            last: point,
            spacing_left: 0.,
            brush,
        }
    }
    pub fn extend(&mut self, doc: &mut Document, point: Point) {
        let dx = point.x - self.last.x;
        let dy = point.y - self.last.y;
        let dist = dx.hypot(dy);
        if !dist.is_finite() || dist == 0. {
            return;
        }
        let spacing = (self.brush.size * 0.12).max(0.5);
        let mut step = if self.spacing_left == 0. {
            spacing
        } else {
            self.spacing_left
        };
        while step <= dist {
            let t = step / dist;
            stamp(
                doc,
                &self.brush,
                Point {
                    x: self.last.x + dx * t,
                    y: self.last.y + dy * t,
                    pressure: self.last.pressure + (point.pressure - self.last.pressure) * t,
                },
            );
            step += spacing;
        }
        self.spacing_left = step - dist;
        self.last = point;
    }
}
fn stamp(doc: &mut Document, brush: &Brush, p: Point) {
    if !p.x.is_finite() || !p.y.is_finite() {
        return;
    }
    let layer = &mut doc.layers[doc.active];
    if layer.locked || !layer.visible {
        return;
    }
    let pressure = if brush.pressure && p.pressure.is_finite() {
        p.pressure.clamp(0.05, 1.)
    } else {
        1.
    };
    let radius = (brush.size * pressure / 2.).clamp(0.5, 256.);
    let pixels = Arc::make_mut(&mut layer.pixels);
    let x0 = ((p.x - radius - 1.).floor() as i32).max(0);
    let y0 = ((p.y - radius - 1.).floor() as i32).max(0);
    let x1 = ((p.x + radius + 1.).ceil() as i32).min(doc.width as i32);
    let y1 = ((p.y + radius + 1.).ceil() as i32).min(doc.height as i32);
    for y in y0..y1 {
        for x in x0..x1 {
            let nx = (x as f32 + 0.5 - p.x) / radius;
            let ny = (y as f32 + 0.5 - p.y) / radius;
            let coverage = match &brush.tip {
                BrushTip::Round => ((1. - nx.hypot(ny)) * radius + 0.5).clamp(0., 1.),
                BrushTip::Soft => (1. - nx.hypot(ny)).clamp(0., 1.).powi(2),
                BrushTip::Image {
                    width,
                    height,
                    mask,
                } => {
                    if nx.abs() > 1. || ny.abs() > 1. {
                        0.
                    } else {
                        let ix = (((nx + 1.) * 0.5 * *width as f32) as u32).min(width - 1);
                        let iy = (((ny + 1.) * 0.5 * *height as f32) as u32).min(height - 1);
                        mask[(iy * width + ix) as usize] as f32 / 255.
                    }
                }
            };
            let alpha = coverage * brush.opacity;
            let idx = ((y as u32 * doc.width + x as u32) * 4) as usize;
            if brush.erase {
                pixels[idx + 3] = (pixels[idx + 3] as f32 * (1. - alpha)).round() as u8;
            } else {
                composite_over(&mut pixels[idx..idx + 4], brush.color, alpha);
            }
        }
    }
    doc.touch();
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continuous_stroke_and_erase() {
        let mut d = Document::new(64, 32);
        let b = Brush {
            size: 8.,
            ..Default::default()
        };
        let mut s = Stroke::begin(
            &mut d,
            b.clone(),
            Point {
                x: 4.,
                y: 16.,
                pressure: 1.,
            },
        );
        s.extend(
            &mut d,
            Point {
                x: 60.,
                y: 16.,
                pressure: 1.,
            },
        );
        for x in 4..60 {
            assert!(d.layers[1].pixels[(16 * 64 + x) * 4 + 3] > 0);
        }
        let mut eraser = b;
        eraser.erase = true;
        Stroke::begin(
            &mut d,
            eraser,
            Point {
                x: 32.,
                y: 16.,
                pressure: 1.,
            },
        );
        assert_eq!(d.layers[1].pixels[(16 * 64 + 32) * 4 + 3], 0);
    }
    #[test]
    fn locked_layer_cannot_be_painted() {
        let mut d = Document::new(8, 8);
        d.active = 0;
        let before = d.clone();
        Stroke::begin(
            &mut d,
            Brush::default(),
            Point {
                x: 4.,
                y: 4.,
                pressure: 1.,
            },
        );
        assert_eq!(d, before);
    }
}
