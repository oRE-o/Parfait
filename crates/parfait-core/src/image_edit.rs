//! Bounded, offline raster editing. View transforms never enter this module.
use crate::{Document, LayerKind, PixelRect};
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PixelEdit {
    Fill([u8; 4]),
    ClearOutside,
    BrightnessContrast {
        brightness: f32,
        contrast: f32,
    },
    Hsl {
        hue: f32,
        saturation: f32,
        lightness: f32,
    },
    Posterize(u8),
    Invert,
    Levels {
        black: u8,
        white: u8,
        gamma: f32,
    },
    Curve([f32; 5]),
    Balance([f32; 3]),
    Threshold(u8),
    Gradient {
        low: [u8; 3],
        high: [u8; 3],
    },
    Statistics {
        source_mean: [f32; 3],
        source_std: [f32; 3],
        target_mean: [f32; 3],
        target_std: [f32; 3],
    },
    Border {
        color: [u8; 4],
        radius: u32,
    },
}

impl PixelEdit {
    pub fn validate(self) -> bool {
        match self {
            Self::Statistics {
                source_mean,
                source_std,
                target_mean,
                target_std,
            } => {
                source_mean
                    .into_iter()
                    .chain(target_mean)
                    .all(|v| v.is_finite() && (0. ..=1.).contains(&v))
                    && source_std
                        .into_iter()
                        .chain(target_std)
                        .all(|v| v.is_finite() && (0.001..=1.).contains(&v))
            }
            Self::BrightnessContrast { brightness, contrast } => {
                brightness.is_finite()
                    && (-1. ..=1.).contains(&brightness)
                    && contrast.is_finite()
                    && (-1. ..=1.).contains(&contrast)
            }
            Self::Hsl { hue, saturation, lightness } => {
                hue.is_finite()
                    && (-180. ..=180.).contains(&hue)
                    && saturation.is_finite()
                    && (-1. ..=1.).contains(&saturation)
                    && lightness.is_finite()
                    && (-1. ..=1.).contains(&lightness)
            }
            Self::Posterize(levels) => levels >= 2,
            Self::Levels { black, white, gamma } => {
                black < white
                    && gamma.is_finite()
                    && (0.1..=10.).contains(&gamma)
            }
            Self::Curve(points) => {
                points.iter().all(|p| p.is_finite() && (0. ..=1.).contains(p))
            }
            Self::Balance(values) => {
                values.iter().all(|p| p.is_finite() && (-1. ..=1.).contains(p))
            }
            Self::Border { radius, .. } => (1..=128).contains(&radius),
            _ => true,
        }
    }
    fn color(self, before: [u8; 4]) -> [u8; 4] {
        let mut result = before;
        let luma = (f32::from(before[0]) * 0.2126
            + f32::from(before[1]) * 0.7152
            + f32::from(before[2]) * 0.0722)
            / 255.;
        if let Self::Fill(color) = self {
            return color;
        }
        if let Self::ClearOutside = self {
            return [0; 4];
        }
        if let Self::Hsl { hue, saturation, lightness } = self {
            if hue == 0. && saturation == 0. && lightness == 0. {
                return before;
            }
            let rgb =
                [before[0], before[1], before[2]].map(|v| f32::from(v) / 255.);
            let max = rgb.iter().copied().fold(0., f32::max);
            let min = rgb.iter().copied().fold(1., f32::min);
            let delta = max - min;
            let mut h = if delta == 0. {
                0.
            } else if max == rgb[0] {
                ((rgb[1] - rgb[2]) / delta).rem_euclid(6.)
            } else if max == rgb[1] {
                (rgb[2] - rgb[0]) / delta + 2.
            } else {
                (rgb[0] - rgb[1]) / delta + 4.
            } / 6.;
            let mut l = (max + min) / 2.;
            let mut s = if delta == 0. {
                0.
            } else {
                delta / (1. - (2. * l - 1.).abs()).max(f32::EPSILON)
            };
            h = (h + hue / 360.).rem_euclid(1.);
            s = (s + saturation).clamp(0., 1.);
            l = (l + lightness).clamp(0., 1.);
            let c = (1. - (2. * l - 1.).abs()) * s;
            let x = c * (1. - ((h * 6.).rem_euclid(2.) - 1.).abs());
            let base = match (h * 6.) as u8 {
                0 => [c, x, 0.],
                1 => [x, c, 0.],
                2 => [0., c, x],
                3 => [0., x, c],
                4 => [x, 0., c],
                _ => [c, 0., x],
            };
            for i in 0..3 {
                result[i] = byte(base[i] + l - c / 2.);
            }
            return result;
        }
        for channel in 0..3 {
            let value = f32::from(before[channel]) / 255.;
            let mapped = match self {
                Self::Statistics {
                    source_mean,
                    source_std,
                    target_mean,
                    target_std,
                } => {
                    (value - source_mean[channel]) * target_std[channel]
                        / source_std[channel]
                        + target_mean[channel]
                }
                Self::BrightnessContrast { brightness, contrast } => {
                    let scale = if contrast >= 0. {
                        1. / (1. - contrast.min(0.999))
                    } else {
                        1. + contrast
                    };
                    (value - 0.5) * scale + 0.5 + brightness
                }
                Self::Posterize(levels) => {
                    (value * f32::from(levels - 1)).round()
                        / f32::from(levels - 1)
                }
                Self::Invert => 1. - value,
                Self::Levels { black, white, gamma } => {
                    ((f32::from(before[channel]) - f32::from(black))
                        / f32::from(white - black))
                    .clamp(0., 1.)
                    .powf(1. / gamma)
                }
                Self::Curve(points) => {
                    let x = value * 4.;
                    let i = (x as usize).min(3);
                    points[i] + (points[i + 1] - points[i]) * (x - i as f32)
                }
                Self::Balance(values) => value + values[channel],
                Self::Threshold(threshold) => {
                    if luma * 255. >= f32::from(threshold) { 1. } else { 0. }
                }
                Self::Gradient { low, high } => {
                    (f32::from(low[channel]) * (1. - luma)
                        + f32::from(high[channel]) * luma)
                        / 255.
                }
                _ => value,
            };
            result[channel] = byte(mapped);
        }
        result
    }
}
fn byte(value: f32) -> u8 {
    (value.clamp(0., 1.) * 255.).round() as u8
}

#[derive(Clone, Debug, PartialEq)]
pub struct PixelClipboard {
    pub width: u32,
    pub height: u32,
    pub origin: [i32; 2],
    pub pixels: Vec<u8>,
}
impl PixelClipboard {
    pub fn validate(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.width <= 4096
            && self.height <= 4096
            && self.pixels.len()
                == self.width as usize * self.height as usize * 4
    }
    /// Exact pixel-run vector geometry; no embedded image, script or external resource.
    pub fn to_svg(&self) -> Result<String, &'static str> {
        use std::fmt::Write;
        if !self.validate() {
            return Err("유효한 이미지가 필요합니다.");
        }
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" shape-rendering=\"crispEdges\">",
            self.width, self.height, self.width, self.height
        );
        for y in 0..self.height {
            let mut x = 0;
            while x < self.width {
                let index = (y * self.width + x) as usize * 4;
                let p = &self.pixels[index..index + 4];
                let start = x;
                x += 1;
                while x < self.width
                    && self.pixels[(y * self.width + x) as usize * 4..][..4]
                        == *p
                {
                    x += 1;
                }
                if p[3] == 0 {
                    continue;
                }
                if svg.len() + 256 > 16 * 1024 * 1024 {
                    return Err(
                        "SVG가 16 MiB를 초과합니다. 더 작은 범위를 선택해 주세요.",
                    );
                }
                write!(svg,"<path d=\"M{start} {y}h{}v1H{start}Z\" fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{:.6}\"/>",x-start,p[0],p[1],p[2],f32::from(p[3])/255.).unwrap();
            }
        }
        svg.push_str("</svg>");
        Ok(svg)
    }
}

impl Document {
    pub fn pixel_edit_reason(
        &self,
        index: usize,
        edit: PixelEdit,
    ) -> Option<&'static str> {
        if !self.validate() || !edit.validate() {
            return Some("유효하지 않은 이미지 또는 편집 값입니다.");
        }
        let Some(layer) = self.layers.get(index) else {
            return Some("편집할 레이어가 없습니다.");
        };
        if self.native_content(index).is_some() {
            return Some("텍스트·벡터를 먼저 래스터화해 주세요 (레이어 메뉴).");
        }
        if layer.kind != LayerKind::Raster {
            return Some("래스터 레이어를 선택해 주세요.");
        }
        if !self.layer_ancestors_unlocked(index) {
            return Some("레이어와 부모 폴더의 잠금을 해제해 주세요.");
        }
        if source_alpha_edit(edit) && layer.alpha_locked {
            return Some("투명 픽셀 잠금을 해제해 주세요.");
        }
        if matches!(edit, PixelEdit::ClearOutside | PixelEdit::Border { .. })
            && self.selection.is_none()
        {
            return Some("선택 범위를 먼저 만들어 주세요.");
        }
        None
    }

    pub fn edit_layer_pixels(&mut self, index: usize, edit: PixelEdit) -> bool {
        self.edit_layer_pixels_cancellable(index, edit, || false)
    }
    pub fn edit_layer_pixels_cancellable(
        &mut self,
        index: usize,
        edit: PixelEdit,
        cancelled: impl Fn() -> bool,
    ) -> bool {
        if self.pixel_edit_reason(index, edit).is_some() {
            return false;
        }
        let border = if let PixelEdit::Border { radius, .. } = edit {
            Some(self.selection_border(radius))
        } else {
            None
        };
        let source = &self.layers[index];
        let mut output = None;
        for (i, pixel) in source.pixels.as_chunks::<4>().0.iter().enumerate() {
            if i.is_multiple_of(self.width as usize) && cancelled() {
                return false;
            }
            let mut coverage = self.selection_value(i);
            let after = match edit {
                PixelEdit::ClearOutside => {
                    coverage = 255 - coverage;
                    [0; 4]
                }
                PixelEdit::Border { color, .. } => {
                    coverage = border.as_ref().unwrap()[i];
                    color
                }
                _ => edit.color(*pixel),
            };
            let mut result =
                crate::selection::interpolate_pixel(*pixel, after, coverage);
            if source.alpha_locked {
                result[3] = pixel[3];
            }
            if result != *pixel {
                output.get_or_insert_with(|| source.pixels.as_ref().clone())
                    [i * 4..i * 4 + 4]
                    .copy_from_slice(&result);
            }
        }
        let Some(output) = output else {
            return false;
        };
        if cancelled() {
            return false;
        }
        self.layers[index].pixels = Arc::new(output);
        self.touch();
        true
    }

    // Square inner border via two sliding-minimum passes, O(width*height).
    fn selection_border(&self, radius: u32) -> Vec<u8> {
        let width = self.width as usize;
        let height = self.height as usize;
        let radius = radius as usize;
        let mut horizontal = vec![0; width * height];
        let mut eroded = vec![0; width * height];
        for y in 0..height {
            sliding_min(
                width,
                radius,
                |x| self.selection_value(y * width + x),
                |x, v| horizontal[y * width + x] = v,
            );
        }
        for x in 0..width {
            sliding_min(
                height,
                radius,
                |y| horizontal[y * width + x],
                |y, v| {
                    eroded[y * width + x] =
                        self.selection_value(y * width + x).saturating_sub(v)
                },
            );
        }
        eroded
    }

    pub fn selection_bounds(&self) -> Option<PixelRect> {
        match self.selection.as_ref()? {
            crate::MaskCoverage::Uniform(0) => return None,
            crate::MaskCoverage::Uniform(_) => {
                return Some(PixelRect {
                    x0: 0,
                    y0: 0,
                    x1: self.width,
                    y1: self.height,
                });
            }
            _ => {}
        }
        bounds(self.width, self.height, |i| self.selection_value(i) > 0)
    }
    pub fn copy_layer_pixels(&self, index: usize) -> Option<PixelClipboard> {
        let layer = self.layers.get(index)?;
        if !self.validate() || layer.kind != LayerKind::Raster {
            return None;
        }
        let rect = if self.selection.is_some() {
            self.selection_bounds()?
        } else {
            bounds(self.width, self.height, |i| layer.pixels[i * 4 + 3] > 0)?
        };
        let mut pixels = Vec::with_capacity(
            ((rect.x1 - rect.x0) * (rect.y1 - rect.y0) * 4) as usize,
        );
        for y in rect.y0..rect.y1 {
            for x in rect.x0..rect.x1 {
                let i = (y * self.width + x) as usize;
                let mut pixel: [u8; 4] =
                    layer.pixels[i * 4..i * 4 + 4].try_into().unwrap();
                pixel[3] = ((u16::from(layer.mask_alpha(i, pixel[3]))
                    * u16::from(self.selection_value(i))
                    + 127)
                    / 255) as u8;
                pixels.extend_from_slice(&pixel);
            }
        }
        Some(PixelClipboard {
            width: rect.x1 - rect.x0,
            height: rect.y1 - rect.y0,
            origin: [rect.x0 as i32, rect.y0 as i32],
            pixels,
        })
    }
    pub fn paste_pixels(
        &mut self,
        clip: &PixelClipboard,
        origin: [i32; 2],
    ) -> bool {
        if !self.validate() || !clip.validate() || self.layers.len() >= 64 {
            return false;
        }
        if i64::from(origin[0]) + i64::from(clip.width) <= 0
            || i64::from(origin[1]) + i64::from(clip.height) <= 0
            || origin[0] >= self.width as i32
            || origin[1] >= self.height as i32
        {
            return false;
        }
        let mut next = self.clone();
        let count = next.layers.len();
        next.add_layer();
        if next.layers.len() != count + 1 {
            return false;
        }
        let pixels = Arc::make_mut(&mut next.layers[next.active].pixels);
        for y in 0..clip.height {
            for x in 0..clip.width {
                let dx = i64::from(origin[0]) + i64::from(x);
                let dy = i64::from(origin[1]) + i64::from(y);
                if dx >= 0
                    && dy >= 0
                    && dx < i64::from(self.width)
                    && dy < i64::from(self.height)
                {
                    let i = (y * clip.width + x) as usize * 4;
                    let d =
                        (dy as usize * self.width as usize + dx as usize) * 4;
                    pixels[d..d + 4].copy_from_slice(&clip.pixels[i..i + 4]);
                }
            }
        }
        next.layers[next.active].name = "붙여넣은 레이어".into();
        if !next.validate() {
            return false;
        }
        *self = next;
        true
    }
}
fn source_alpha_edit(edit: PixelEdit) -> bool {
    matches!(edit, PixelEdit::ClearOutside)
}
pub(crate) fn bounds(
    width: u32,
    height: u32,
    contains: impl Fn(usize) -> bool,
) -> Option<PixelRect> {
    let mut rect = PixelRect { x0: width, y0: height, x1: 0, y1: 0 };
    for y in 0..height {
        for x in 0..width {
            if contains((y * width + x) as usize) {
                rect.x0 = rect.x0.min(x);
                rect.y0 = rect.y0.min(y);
                rect.x1 = rect.x1.max(x + 1);
                rect.y1 = rect.y1.max(y + 1);
            }
        }
    }
    (rect.x0 < rect.x1 && rect.y0 < rect.y1).then_some(rect)
}
fn sliding_min(
    len: usize,
    radius: usize,
    read: impl Fn(usize) -> u8,
    mut write: impl FnMut(usize, u8),
) {
    let mut queue = std::collections::VecDeque::<(usize, u8)>::new();
    for i in 0..len + radius {
        let value = if i < len { read(i) } else { 0 };
        while queue.back().is_some_and(|(_, v)| *v >= value) {
            queue.pop_back();
        }
        queue.push_back((i, value));
        if i >= radius {
            let center = i - radius;
            while queue.front().is_some_and(|(j, _)| *j + 2 * radius < i) {
                queue.pop_front();
            }
            write(
                center,
                if center < radius { 0 } else { queue.front().unwrap().1 },
            );
        }
    }
}
