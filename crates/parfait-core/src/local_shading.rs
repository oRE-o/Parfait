//! Silhouette-distance lighting, independently implemented for offline editing.
//! It approximates rounded surfaces; it does not infer anatomy, depth or cast shadows.
use crate::{BlendMode, Document, PixelEdit};
use std::sync::Arc;
#[cfg(test)]
mod tests;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalShading {
    pub angle: f32,
    pub elevation: f32,
    pub radius: f32,
    pub strength: f32,
    /// 0 for smooth shading, 2..4 for discrete bands.
    pub bands: u8,
    pub shadow: [u8; 3],
    pub highlight: [u8; 3],
    pub point_light: bool,
    pub position: [f32; 2],
    pub line_influence: f32,
}
impl Default for LocalShading {
    fn default() -> Self {
        Self {
            angle: -45.,
            elevation: 35.,
            radius: 48.,
            strength: 0.6,
            bands: 0,
            shadow: [64, 62, 100],
            highlight: [255, 231, 193],
            point_light: false,
            position: [0.25, 0.25],
            line_influence: 0.,
        }
    }
}
impl LocalShading {
    pub fn validate(self) -> bool {
        self.angle.is_finite()
            && (-180. ..=180.).contains(&self.angle)
            && self.elevation.is_finite()
            && (5. ..=85.).contains(&self.elevation)
            && self.radius.is_finite()
            && (1. ..=512.).contains(&self.radius)
            && self.strength.is_finite()
            && (0. ..=1.).contains(&self.strength)
            && self.line_influence.is_finite()
            && (0. ..=1.).contains(&self.line_influence)
            && (self.bands == 0 || (2..=4).contains(&self.bands))
            && self
                .position
                .into_iter()
                .all(|v| v.is_finite() && (-1. ..=2.).contains(&v))
    }
}
impl Document {
    pub fn local_shading_reason(
        &self,
        index: usize,
        options: LocalShading,
    ) -> Option<&'static str> {
        if !options.validate() {
            return Some("음영 설정이 유효하지 않습니다.");
        }
        if let Some(reason) = self.pixel_edit_reason(index, PixelEdit::Invert) {
            return Some(reason);
        }
        if self.layers[index].clipped {
            return Some("클리핑되지 않은 밑색 레이어를 선택해 주세요.");
        }
        if self.layers.len() > 62
            || !self.can_allocate_layers(2)
            || self.logical_pixel_bytes().saturating_add(
                u64::from(self.width) * u64::from(self.height) * 8,
            ) > 512 * 1024 * 1024
        {
            return Some("음영 레이어 두 개를 만들 메모리가 부족합니다.");
        }
        None
    }
    pub fn add_local_shading(
        &mut self,
        index: usize,
        options: LocalShading,
    ) -> bool {
        self.add_local_shading_cancellable(index, options, || false)
    }
    pub fn add_local_shading_cancellable(
        &mut self,
        index: usize,
        options: LocalShading,
        cancelled: impl Fn() -> bool,
    ) -> bool {
        if self.local_shading_reason(index, options).is_some()
            || options.strength == 0.
        {
            return false;
        }
        let w = self.width as usize;
        let h = self.height as usize;
        let n = w * h;
        let source = &self.layers[index];
        let mut distance = vec![0_f32; n];
        let mut any = false;
        for y in 0..h {
            if cancelled() {
                return false;
            }
            for x in 0..w {
                let i = y * w + x;
                if source.pixels[i * 4 + 3] > 0 {
                    any = true;
                    distance[i] = options
                        .radius
                        .min((x + 1).min(y + 1).min(w - x).min(h - y) as f32);
                }
            }
        }
        if !any {
            return false;
        }
        for y in 0..h {
            if cancelled() {
                return false;
            }
            for x in 0..w {
                let i = y * w + x;
                let mut d = distance[i];
                if x > 0 {
                    d = d.min(distance[i - 1] + 1.);
                }
                if y > 0 {
                    d = d.min(distance[i - w] + 1.);
                    if x > 0 {
                        d = d.min(
                            distance[i - w - 1] + std::f32::consts::SQRT_2,
                        );
                    }
                    if x + 1 < w {
                        d = d.min(
                            distance[i - w + 1] + std::f32::consts::SQRT_2,
                        );
                    }
                }
                distance[i] = d;
            }
        }
        for y in (0..h).rev() {
            if cancelled() {
                return false;
            }
            for x in (0..w).rev() {
                let i = y * w + x;
                let mut d = distance[i];
                if x + 1 < w {
                    d = d.min(distance[i + 1] + 1.);
                }
                if y + 1 < h {
                    d = d.min(distance[i + w] + 1.);
                    if x > 0 {
                        d = d.min(
                            distance[i + w - 1] + std::f32::consts::SQRT_2,
                        );
                    }
                    if x + 1 < w {
                        d = d.min(
                            distance[i + w + 1] + std::f32::consts::SQRT_2,
                        );
                    }
                }
                distance[i] = d;
            }
        }
        for (i, d) in distance.iter_mut().enumerate() {
            if i.is_multiple_of(w) && cancelled() {
                return false;
            }
            let t = (*d / options.radius).clamp(0., 1.);
            *d = (1. - (1. - t).powi(2)).sqrt() * options.radius;
            if options.line_influence > 0. {
                let mut ink = 0_f32;
                for (l, layer) in
                    self.layers.iter().enumerate().filter(|(l, layer)| {
                        *l != index
                            && layer.reference
                            && layer.visible
                            && layer.kind == crate::LayerKind::Raster
                    })
                {
                    let _ = l;
                    let p = &layer.pixels[i * 4..i * 4 + 4];
                    let light = (f32::from(p[0]) * 0.2126
                        + f32::from(p[1]) * 0.7152
                        + f32::from(p[2]) * 0.0722)
                        / 255.;
                    ink = ink.max(
                        (1. - light) * f32::from(layer.mask_alpha(i, p[3]))
                            / 255.
                            * layer.opacity,
                    );
                }
                *d -= ink * options.line_influence * 3.;
            }
        }
        // Smooth distance-field ridges before differentiating. Two sliding box
        // passes are linear in canvas size and independent of the blur radius.
        let r = (options.radius / 8.).round().clamp(1., 32.) as usize;
        let count = (2 * r + 1) as f32;
        let mut temp = vec![0_f32; n];
        for y in 0..h {
            if cancelled() {
                return false;
            }
            let mut sum =
                (0..=r.min(w - 1)).map(|x| distance[y * w + x]).sum::<f32>();
            for x in 0..w {
                temp[y * w + x] = sum / count;
                if x + r + 1 < w {
                    sum += distance[y * w + x + r + 1];
                }
                if x >= r {
                    sum -= distance[y * w + x - r];
                }
            }
        }
        for x in 0..w {
            if cancelled() {
                return false;
            }
            let mut sum =
                (0..=r.min(h - 1)).map(|y| temp[y * w + x]).sum::<f32>();
            for y in 0..h {
                distance[y * w + x] = (sum / count).max(0.);
                if y + r + 1 < h {
                    sum += temp[(y + r + 1) * w + x];
                }
                if y >= r {
                    sum -= temp[(y - r) * w + x];
                }
            }
        }
        drop(temp);
        let (sin, cos) = options.angle.to_radians().sin_cos();
        let elev = options.elevation.to_radians();
        let directional = [cos * elev.cos(), sin * elev.cos(), elev.sin()];
        let mut shadow = vec![0; n * 4];
        let mut highlight = vec![0; n * 4];
        for y in 0..h {
            if cancelled() {
                return false;
            }
            for x in 0..w {
                let i = y * w + x;
                if source.pixels[i * 4 + 3] == 0 {
                    continue;
                }
                let dx = (distance[y * w + (x + 1).min(w - 1)]
                    - distance[y * w + x.saturating_sub(1)])
                    / 2.;
                let dy = (distance[(y + 1).min(h - 1) * w + x]
                    - distance[y.saturating_sub(1) * w + x])
                    / 2.;
                let norm = (dx * dx + dy * dy + 1.).sqrt();
                let light = if options.point_light {
                    let lx = options.position[0] * w as f32 - x as f32;
                    let ly = options.position[1] * h as f32 - y as f32;
                    let lz = elev.tan() * (w.min(h) as f32).max(1.);
                    let length = (lx * lx + ly * ly + lz * lz).sqrt();
                    [lx / length, ly / length, lz / length]
                } else {
                    directional
                };
                let diffuse = ((-dx * light[0] - dy * light[1] + light[2])
                    / norm)
                    .clamp(0., 1.);
                let mut dark = ((0.72 - diffuse) / 0.72).clamp(0., 1.);
                let mut bright = ((diffuse - 0.72) / 0.28).clamp(0., 1.);
                if options.bands > 0 {
                    let levels = f32::from(options.bands - 1);
                    dark = (dark * levels).round() / levels;
                    bright = (bright * levels).round() / levels;
                }
                let coverage = f32::from(self.selection_value(i)) / 255.;
                for (output, color, amount) in [
                    (&mut shadow, options.shadow, dark),
                    (&mut highlight, options.highlight, bright),
                ] {
                    output[i * 4..i * 4 + 3].copy_from_slice(&color);
                    output[i * 4 + 3] =
                        (amount * coverage * options.strength * 255.).round()
                            as u8;
                }
            }
        }
        if cancelled()
            || !shadow
                .as_chunks::<4>()
                .0
                .iter()
                .chain(highlight.as_chunks::<4>().0)
                .any(|p| p[3] > 0)
        {
            return false;
        }
        let mut next = self.clone();
        next.active = index;
        for (pixels, name, blend) in [
            (shadow, "음영 · 암부", BlendMode::Multiply),
            (highlight, "음영 · 명부", BlendMode::Screen),
        ] {
            let count = next.layers.len();
            next.add_layer();
            if next.layers.len() != count + 1 {
                return false;
            }
            let layer = &mut next.layers[next.active];
            layer.name = name.into();
            layer.blend_mode = blend;
            layer.clipped = true;
            layer.pixels = Arc::new(pixels);
        }
        if !next.validate() || cancelled() {
            return false;
        }
        *self = next;
        true
    }
}
