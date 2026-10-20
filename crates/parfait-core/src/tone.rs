//! Independent, bounded local color adjustments. No model or network resources.
use crate::{Document, PixelEdit, ResponseCurve};
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneLevels {
    pub black: u8,
    pub white: u8,
    pub gamma: f32,
    pub output_black: u8,
    pub output_white: u8,
}
impl Default for ToneLevels {
    fn default() -> Self {
        Self {
            black: 0,
            white: 255,
            gamma: 1.,
            output_black: 0,
            output_white: 255,
        }
    }
}
impl ToneLevels {
    pub fn sample(self, value: f32) -> f32 {
        let t = ((value * 255. - f32::from(self.black))
            / f32::from(self.white - self.black))
        .clamp(0., 1.)
        .powf(1. / self.gamma);
        (f32::from(self.output_black)
            + t * (f32::from(self.output_white) - f32::from(self.output_black)))
            / 255.
    }
    fn validate(self) -> bool {
        self.black < self.white
            && self.gamma.is_finite()
            && (0.1..=10.).contains(&self.gamma)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct GradientStop {
    pub position: f32,
    pub color: [u8; 3],
    /// Relative position at which this segment is half mixed.
    pub midpoint: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub enum ToneAdjustment {
    /// Master RGB first, then the independent red, green and blue channels.
    Levels([ToneLevels; 4]),
    Curves([ResponseCurve; 4]),
    Balance {
        ranges: [[f32; 3]; 3],
        preserve_luminance: bool,
    },
    Gradient {
        stops: Vec<GradientStop>,
        reverse: bool,
        linear_light: bool,
    },
    Hsv {
        hue: f32,
        saturation: f32,
        value: f32,
    },
    Threshold {
        value: u8,
        preserve_alpha: bool,
    },
}
fn luma(rgb: [f32; 3]) -> f32 {
    rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722
}
fn byte(v: f32) -> u8 {
    (v.clamp(0., 1.) * 255.).round() as u8
}
fn srgb_decode(v: f32) -> f32 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}
fn srgb_encode(v: f32) -> f32 {
    if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1. / 2.4) - 0.055 }
}
struct GradientSegment {
    start: f32,
    end: f32,
    a: [f32; 3],
    b: [f32; 3],
    exponent: f32,
}
struct PreparedGradient {
    segments: Vec<GradientSegment>,
    reverse: bool,
    linear: bool,
}
impl PreparedGradient {
    fn new(edit: &ToneAdjustment) -> Option<Self> {
        let ToneAdjustment::Gradient { stops, reverse, linear_light } = edit
        else {
            return None;
        };
        Some(Self {
            segments: stops
                .windows(2)
                .map(|s| {
                    let color = |c: [u8; 3]| {
                        c.map(|v| {
                            let v = f32::from(v) / 255.;
                            if *linear_light { srgb_decode(v) } else { v }
                        })
                    };
                    GradientSegment {
                        start: s[0].position,
                        end: s[1].position,
                        a: color(s[0].color),
                        b: color(s[1].color),
                        exponent: 0.5_f32.ln() / s[0].midpoint.ln(),
                    }
                })
                .collect(),
            reverse: *reverse,
            linear: *linear_light,
        })
    }
    fn color(&self, input: f32) -> [u8; 3] {
        let t = if self.reverse { 1. - input } else { input }.clamp(0., 1.);
        let i = self
            .segments
            .partition_point(|s| s.start <= t)
            .saturating_sub(1)
            .min(self.segments.len() - 1);
        let s = &self.segments[i];
        let mix =
            ((t - s.start) / (s.end - s.start)).clamp(0., 1.).powf(s.exponent);
        std::array::from_fn(|c| {
            let v = s.a[c] * (1. - mix) + s.b[c] * mix;
            byte(if self.linear { srgb_encode(v) } else { v })
        })
    }
}
/// Preserve linear RGB-weighted luminance while bringing out-of-gamut colors
/// toward neutral. This avoids independent clipping changing the target lightness.
pub fn preserve_luminance(mut rgb: [f32; 3], target: f32) -> [f32; 3] {
    let delta = target - luma(rgb);
    for c in &mut rgb {
        *c += delta;
    }
    let mut scale = 1_f32;
    for c in rgb {
        if c > 1. {
            scale = scale.min((1. - target) / (c - target));
        }
        if c < 0. {
            scale = scale.min(target / (target - c));
        }
    }
    rgb.map(|c| (target + (c - target) * scale).clamp(0., 1.))
}
impl ToneAdjustment {
    pub fn validate(&self) -> bool {
        match self {
            Self::Levels(levels) => levels.iter().all(|v| v.validate()),
            Self::Curves(curves) => curves.iter().all(ResponseCurve::validate),
            Self::Balance { ranges, .. } => ranges
                .iter()
                .flatten()
                .all(|v| v.is_finite() && (-1. ..=1.).contains(v)),
            Self::Gradient { stops, .. } => {
                (2..=32).contains(&stops.len())
                    && stops.first().is_some_and(|s| s.position == 0.)
                    && stops.last().is_some_and(|s| s.position == 1.)
                    && stops.iter().all(|s| {
                        s.position.is_finite()
                            && (0. ..=1.).contains(&s.position)
                            && s.midpoint.is_finite()
                            && (0.01..=0.99).contains(&s.midpoint)
                    })
                    && stops
                        .windows(2)
                        .all(|s| s[1].position - s[0].position >= 0.001)
            }
            Self::Hsv { hue, saturation, value } => {
                hue.is_finite()
                    && (-180. ..=180.).contains(hue)
                    && [saturation, value]
                        .into_iter()
                        .all(|v| v.is_finite() && (-1. ..=1.).contains(v))
            }
            Self::Threshold { .. } => true,
        }
    }
    pub fn gradient_color(&self, input: f32) -> [u8; 3] {
        let Self::Gradient { stops, reverse, linear_light } = self else {
            return [0; 3];
        };
        let t = if *reverse { 1. - input } else { input }.clamp(0., 1.);
        let i = stops
            .partition_point(|s| s.position <= t)
            .saturating_sub(1)
            .min(stops.len() - 2);
        let (a, b) = (&stops[i], &stops[i + 1]);
        let u = ((t - a.position) / (b.position - a.position)).clamp(0., 1.);
        let mix = u.powf(0.5_f32.ln() / a.midpoint.ln());
        std::array::from_fn(|c| {
            let (a, b) =
                (f32::from(a.color[c]) / 255., f32::from(b.color[c]) / 255.);
            let decode = |v: f32| {
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            let encode = |v: f32| {
                if v <= 0.0031308 {
                    v * 12.92
                } else {
                    1.055 * v.powf(1. / 2.4) - 0.055
                }
            };
            byte(if *linear_light {
                encode(decode(a) * (1. - mix) + decode(b) * mix)
            } else {
                a * (1. - mix) + b * mix
            })
        })
    }
    fn color(
        &self,
        before: [u8; 4],
        lut: &[[u8; 256]; 3],
        gradient: Option<&PreparedGradient>,
    ) -> [u8; 4] {
        let mut after = before;
        let rgb =
            [before[0], before[1], before[2]].map(|c| f32::from(c) / 255.);
        let light = luma(rgb);
        let output = match self {
            Self::Levels(_) | Self::Curves(_) => {
                std::array::from_fn(|c| lut[c][before[c] as usize])
            }
            Self::Gradient { .. } => gradient.unwrap().color(light),
            Self::Threshold { value, preserve_alpha } => {
                if !*preserve_alpha {
                    after[3] = 255;
                }
                [if light * 255. >= f32::from(*value) { 255 } else { 0 }; 3]
            }
            Self::Balance { ranges, preserve_luminance: keep } => {
                let weights = [
                    (1. - 2. * light).max(0.),
                    1. - (2. * light - 1.).abs(),
                    (2. * light - 1.).max(0.),
                ];
                let mut changed = std::array::from_fn(|c| {
                    rgb[c]
                        + (0..3).map(|r| ranges[r][c] * weights[r]).sum::<f32>()
                });
                if *keep {
                    changed = preserve_luminance(changed, light);
                }
                changed.map(byte)
            }
            Self::Hsv { hue, saturation, value } => {
                let max = rgb.into_iter().fold(0., f32::max);
                let min = rgb.into_iter().fold(1., f32::min);
                let delta = max - min;
                let h = if delta == 0. {
                    0.
                } else if max == rgb[0] {
                    ((rgb[1] - rgb[2]) / delta).rem_euclid(6.)
                } else if max == rgb[1] {
                    (rgb[2] - rgb[0]) / delta + 2.
                } else {
                    (rgb[0] - rgb[1]) / delta + 4.
                };
                let h = (h + hue / 60.).rem_euclid(6.);
                let s = if max == 0. { 0. } else { delta / max };
                let s = if *saturation >= 0. {
                    (s / (1. - saturation.min(0.999))).clamp(0., 1.)
                } else {
                    s * (1. + saturation)
                };
                let v = max;
                let c = v * s;
                let x = c * (1. - (h.rem_euclid(2.) - 1.).abs());
                let base = match h as u8 {
                    0 => [c, x, 0.],
                    1 => [x, c, 0.],
                    2 => [0., c, x],
                    3 => [0., x, c],
                    4 => [x, 0., c],
                    _ => [c, 0., x],
                };
                base.map(|c| {
                    let rgb = c + v - v * s;
                    byte(if *value >= 0. {
                        rgb + (1. - rgb) * value
                    } else {
                        rgb * (1. + value)
                    })
                })
            }
        };
        after[..3].copy_from_slice(&output);
        after
    }
}
impl Document {
    pub fn tone_edit_reason(
        &self,
        layers: u64,
        edit: &ToneAdjustment,
    ) -> Option<&'static str> {
        if !edit.validate() {
            return Some("색조 보정 값이 유효하지 않습니다.");
        }
        if layers == 0
            || (self.layers.len() < 64 && layers >> self.layers.len() != 0)
        {
            return Some("보정할 레이어를 선택해 주세요.");
        }
        for (index, layer) in self
            .layers
            .iter()
            .enumerate()
            .filter(|(i, _)| layers & (1_u64 << i) != 0)
        {
            if let Some(reason) =
                self.pixel_edit_reason(index, PixelEdit::Invert)
            {
                return Some(reason);
            }
            if layer.alpha_locked
                && matches!(
                    edit,
                    ToneAdjustment::Threshold { preserve_alpha: false, .. }
                )
            {
                return Some(
                    "투명도를 바꾸려면 투명 픽셀 잠금을 해제해 주세요.",
                );
            }
        }
        None
    }
    pub fn adjust_layer_tones(
        &mut self,
        layers: u64,
        edit: &ToneAdjustment,
    ) -> bool {
        self.adjust_layer_tones_cancellable(layers, edit, || false)
    }
    pub fn adjust_layer_tones_cancellable(
        &mut self,
        layers: u64,
        edit: &ToneAdjustment,
        cancelled: impl Fn() -> bool,
    ) -> bool {
        if self.tone_edit_reason(layers, edit).is_some() {
            return false;
        }
        let lut: [[u8; 256]; 3] = std::array::from_fn(|c| {
            std::array::from_fn(|v| {
                let x = v as f32 / 255.;
                byte(match edit {
                    ToneAdjustment::Levels(l) => {
                        l[c + 1].sample(l[0].sample(x))
                    }
                    ToneAdjustment::Curves(curves) => {
                        curves[c + 1].sample(curves[0].sample(x))
                    }
                    _ => x,
                })
            })
        });
        let mut next = self.clone();
        let gradient = PreparedGradient::new(edit);
        let mut changed = false;
        for (index, layer) in self
            .layers
            .iter()
            .enumerate()
            .filter(|(i, _)| layers & (1_u64 << i) != 0)
        {
            let mut output = None;
            for (i, p) in layer.pixels.as_chunks::<4>().0.iter().enumerate() {
                if i.is_multiple_of(self.width as usize) && cancelled() {
                    return false;
                }
                // Transparent padding is not newly painted by color adjustments.
                if p[3] == 0 {
                    continue;
                }
                let after = crate::selection::interpolate_pixel(
                    *p,
                    edit.color(*p, &lut, gradient.as_ref()),
                    self.selection_value(i),
                );
                if after != *p {
                    output.get_or_insert_with(|| layer.pixels.as_ref().clone())
                        [i * 4..i * 4 + 4]
                        .copy_from_slice(&after);
                }
            }
            if let Some(output) = output {
                next.layers[index].pixels = Arc::new(output);
                changed = true;
            }
        }
        if cancelled() || !changed {
            return false;
        }
        next.touch();
        *self = next;
        true
    }
    /// Alpha × selection weighted histogram. Master channel is luminance.
    pub fn tone_histogram(&self, layers: u64) -> [[u64; 256]; 4] {
        let mut bins = [[0_u64; 256]; 4];
        if !self.validate() {
            return bins;
        }
        for (_, layer) in self.layers.iter().enumerate().filter(|(i, l)| {
            layers & (1_u64 << i) != 0 && l.kind == crate::LayerKind::Raster
        }) {
            for (i, p) in layer.pixels.as_chunks::<4>().0.iter().enumerate() {
                let weight = u64::from(layer.mask_alpha(i, p[3]))
                    * u64::from(self.selection_value(i));
                let light =
                    byte(luma([p[0], p[1], p[2]].map(|c| f32::from(c) / 255.)));
                for (channel, value) in
                    [light, p[0], p[1], p[2]].into_iter().enumerate()
                {
                    bins[channel][value as usize] += weight;
                }
            }
        }
        bins
    }
}
/// Clip 0.5% at either tail; constant/empty channels keep the identity range.
pub fn automatic_tone_range(bins: &[u64; 256]) -> (u8, u8) {
    let total: u64 = bins.iter().sum();
    if total == 0 {
        return (0, 255);
    }
    let cutoff = total / 200;
    let mut sum = 0;
    let low = bins
        .iter()
        .position(|n| {
            sum += n;
            sum > cutoff
        })
        .unwrap_or(0) as u8;
    sum = 0;
    let high = 255
        - bins
            .iter()
            .rev()
            .position(|n| {
                sum += n;
                sum > cutoff
            })
            .unwrap_or(0) as u8;
    if low >= high { (0, 255) } else { (low, high) }
}
