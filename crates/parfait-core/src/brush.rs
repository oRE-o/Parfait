use crate::{Document, PixelRect, composite_over};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
}
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BrushTip {
    Round,
    Soft,
    Pencil,
    Flat,
    Image {
        width: u32,
        height: u32,
        mask: Arc<Vec<u8>>,
    },
}
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Brush {
    pub size: f32,
    pub opacity: f32,
    pub color: [u8; 3],
    pub erase: bool,
    pub pressure: bool,
    pub tip: BrushTip,
    pub pressure_curve: [f32; 5],
    pub minimum_size: f32,
    pub pressure_opacity: bool,
    pub flow: f32,
    pub spacing: f32,
    pub hardness: f32,
    pub aspect: f32,
    pub angle: f32,
    pub smoothing: f32,
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
            pressure_curve: [0., 0.25, 0.5, 0.75, 1.],
            minimum_size: 0.05,
            pressure_opacity: false,
            flow: 1.,
            spacing: 0.12,
            hardness: 1.,
            aspect: 1.,
            angle: 0.,
            smoothing: 0.,
        }
    }
}
impl Brush {
    pub fn mapped_pressure(&self, raw: f32) -> f32 {
        let p = if raw.is_finite() {
            raw.clamp(0., 1.)
        } else {
            1.
        };
        let points = std::array::from_fn::<_, 5, _>(|i| [i as f32 / 4., self.pressure_curve[i]]);
        crate::sample_response(&points, p)
    }
    pub fn validate(&self) -> bool {
        let bounded = |v: f32, a, b| v.is_finite() && (a..=b).contains(&v);
        bounded(self.size, 1., 512.)
            && bounded(self.opacity, 0., 1.)
            && bounded(self.flow, 0.01, 1.)
            && bounded(self.spacing, 0.01, 2.)
            && bounded(self.hardness, 0., 1.)
            && bounded(self.aspect, 0.1, 1.)
            && bounded(self.angle, -180., 180.)
            && bounded(self.smoothing, 0., 0.95)
            && bounded(self.minimum_size, 0., 1.)
            && self.pressure_curve.iter().all(|&v| bounded(v, 0., 1.))
            && self.pressure_curve.windows(2).all(|w| w[0] <= w[1])
            && match &self.tip {
                BrushTip::Image {
                    width,
                    height,
                    mask,
                } => {
                    *width > 0
                        && *height > 0
                        && *width <= 256
                        && *height <= 256
                        && mask.len() == (*width as usize * *height as usize)
                }
                _ => true,
            }
    }
}
pub struct Stroke {
    damage: Option<PixelRect>,
    last: Point,
    spacing_left: f32,
    brush: Brush,
}
impl Stroke {
    pub fn begin(doc: &mut Document, brush: Brush, point: Point) -> Self {
        let damage = stamp(doc, &brush, point);
        Self {
            damage,
            last: point,
            spacing_left: 0.,
            brush,
        }
    }
    /// Drain all dab bounds since the last renderer update.
    pub fn take_damage(&mut self) -> Option<PixelRect> {
        self.damage.take()
    }
    pub fn extend(&mut self, doc: &mut Document, point: Point) {
        let point = Point {
            x: self.last.x + (point.x - self.last.x) * (1. - self.brush.smoothing),
            y: self.last.y + (point.y - self.last.y) * (1. - self.brush.smoothing),
            ..point
        };
        let dx = point.x - self.last.x;
        let dy = point.y - self.last.y;
        let dist = dx.hypot(dy);
        if !dist.is_finite() || dist == 0. {
            return;
        }
        let spacing = (self.brush.size * self.brush.spacing).max(0.5);
        let mut step = if self.spacing_left == 0. {
            spacing
        } else {
            self.spacing_left
        };
        while step <= dist {
            let t = step / dist;
            let damage = stamp(
                doc,
                &self.brush,
                Point {
                    x: self.last.x + dx * t,
                    y: self.last.y + dy * t,
                    pressure: self.last.pressure + (point.pressure - self.last.pressure) * t,
                },
            );
            if let Some(r) = damage {
                self.damage = Some(self.damage.map_or(r, |old| old.union(r)));
            }
            step += spacing;
        }
        self.spacing_left = step - dist;
        self.last = point;
    }
}
fn stamp(doc: &mut Document, brush: &Brush, p: Point) -> Option<PixelRect> {
    if !p.x.is_finite() || !p.y.is_finite() {
        return None;
    }
    let layer = &mut doc.layers[doc.active];
    if layer.locked || !layer.visible {
        return None;
    }
    let mapped = brush.mapped_pressure(p.pressure);
    let pressure = if brush.pressure {
        brush.minimum_size + (1. - brush.minimum_size) * mapped
    } else {
        1.
    };
    let radius = (brush.size * pressure / 2.).clamp(0.5, 256.);
    let x0 = ((p.x - radius - 1.).floor() as i32).max(0);
    let y0 = ((p.y - radius - 1.).floor() as i32).max(0);
    let x1 = ((p.x + radius + 1.).ceil() as i32).min(doc.width as i32);
    let y1 = ((p.y + radius + 1.).ceil() as i32).min(doc.height as i32);
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let pixels = Arc::make_mut(&mut layer.pixels);
    let (sin, cos) = brush.angle.to_radians().sin_cos();
    for y in y0..y1 {
        for x in x0..x1 {
            let dx = (x as f32 + 0.5 - p.x) / radius;
            let dy = (y as f32 + 0.5 - p.y) / radius;
            let nx = cos * dx + sin * dy;
            let ny = (-sin * dx + cos * dy) / brush.aspect;
            let coverage = match &brush.tip {
                BrushTip::Round => {
                    ((1. - nx.hypot(ny)) / (1. - brush.hardness).max(1. / radius)).clamp(0., 1.)
                }
                BrushTip::Soft => (1. - nx.hypot(ny)).clamp(0., 1.).powi(2),
                BrushTip::Pencil => {
                    ((1. - nx.hypot(ny)) * radius).clamp(0., 1.)
                        * (0.25
                            + ((x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263)) as u32 % 100)
                                as f32
                                / 133.)
                }
                BrushTip::Flat => ((1. - nx.abs().max(ny.abs())) * radius).clamp(0., 1.),
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
            let alpha = coverage
                * brush.opacity
                * brush.flow
                * if brush.pressure_opacity { mapped } else { 1. };
            let idx = ((y as u32 * doc.width + x as u32) * 4) as usize;
            if brush.erase {
                pixels[idx + 3] = (pixels[idx + 3] as f32 * (1. - alpha)).round() as u8;
            } else {
                composite_over(&mut pixels[idx..idx + 4], brush.color, alpha);
            }
        }
    }
    doc.touch();
    Some(PixelRect {
        x0: x0 as u32,
        y0: y0 as u32,
        x1: x1 as u32,
        y1: y1 as u32,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pressure_curve_controls_coverage_and_opacity() {
        let mut low = Document::new(64, 64);
        let mut high = low.clone();
        let b = Brush {
            size: 40.,
            pressure_curve: [0., 0.06, 0.25, 0.56, 1.],
            pressure_opacity: true,
            ..Default::default()
        };
        assert!(b.validate());
        assert_eq!(b.mapped_pressure(0.5), 0.25);
        Stroke::begin(
            &mut low,
            b.clone(),
            Point {
                x: 32.,
                y: 32.,
                pressure: 0.5,
            },
        );
        Stroke::begin(
            &mut high,
            b,
            Point {
                x: 32.,
                y: 32.,
                pressure: 1.,
            },
        );
        let alpha = |d: &Document| {
            d.layers[1]
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3] as u64)
                .sum::<u64>()
        };
        assert!(alpha(&high) > alpha(&low) * 5);
        let invalid = Brush {
            pressure_curve: [0., 0.8, 0.1, 0.9, 1.],
            ..Default::default()
        };
        assert!(!invalid.validate());
    }
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
