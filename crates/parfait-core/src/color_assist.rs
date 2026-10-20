//! Deterministic local color assistance; no model, account or remote service.
use crate::{Document, LayerKind, PixelClipboard, PixelEdit};
use std::sync::Arc;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub struct RegionColors {
    pub threshold: u8,
    pub minimum_area: u32,
    pub include_open_regions: bool,
    pub palette: Vec<[u8; 3]>,
}
impl RegionColors {
    pub fn validate(&self) -> bool {
        self.threshold >= 1
            && self.minimum_area > 0
            && self.minimum_area <= 4096 * 4096
            && !self.palette.is_empty()
            && self.palette.len() <= 16
    }
}
impl Document {
    pub fn region_color_reason(
        &self,
        options: &RegionColors,
        hint: Option<&PixelClipboard>,
    ) -> Option<&'static str> {
        if !self.validate()
            || !options.validate()
            || hint.is_some_and(|i| !i.validate())
        {
            return Some("채색 조건이나 힌트 이미지가 유효하지 않습니다.");
        }
        if self.layers.len() >= 64
            || !self.can_allocate_layers(1)
            || self.logical_pixel_bytes()
                + u64::from(self.width) * u64::from(self.height) * 4
                > 512 * 1024 * 1024
        {
            return Some("새 채색 레이어를 위한 용량이 부족합니다.");
        }
        let active = &self.layers[self.active];
        let parent = if active.kind == LayerKind::Folder {
            Some(active.id)
        } else {
            active.parent
        };
        self.folder_edit_reason(parent)
    }
    /// Connected light regions are colored, excluding open regions unless requested.
    pub fn color_regions(
        &mut self,
        options: &RegionColors,
        hint: Option<&PixelClipboard>,
    ) -> bool {
        if self.region_color_reason(options, hint).is_some() {
            return false;
        }
        let image = self.composite();
        let count = (self.width * self.height) as usize;
        let mut visited = vec![false; count];
        let mut output = vec![0; count * 4];
        let mut region = Vec::<u32>::new();
        let mut changed = false;
        let open = |i: usize| {
            let p = &image[i * 4..i * 4 + 4];
            self.selection_value(i) > 0
                && (p[3] < 32
                    || (u32::from(p[0]) * 2126
                        + u32::from(p[1]) * 7152
                        + u32::from(p[2]) * 722)
                        / 10000
                        >= u32::from(options.threshold))
        };
        for first in 0..count {
            if visited[first] {
                continue;
            }
            visited[first] = true;
            if !open(first) {
                continue;
            }
            region.clear();
            region.push(first as u32);
            let mut cursor = 0;
            let mut edge = false;
            let mut hint_sum = [0u64; 3];
            let mut hint_weight = 0u64;
            while cursor < region.len() {
                let i = region[cursor];
                cursor += 1;
                let x = i % self.width;
                let y = i / self.width;
                edge |= x == 0
                    || y == 0
                    || x + 1 == self.width
                    || y + 1 == self.height;
                if let Some(hint) = hint {
                    let hx = u64::from(x) * u64::from(hint.width)
                        / u64::from(self.width);
                    let hy = u64::from(y) * u64::from(hint.height)
                        / u64::from(self.height);
                    let pixel = &hint.pixels[(hy as usize
                        * hint.width as usize
                        + hx as usize)
                        * 4..][..4];
                    let weight = u64::from(pixel[3]);
                    hint_weight += weight;
                    for c in 0..3 {
                        hint_sum[c] += u64::from(pixel[c]) * weight;
                    }
                }
                for neighbor in [
                    x.checked_sub(1).map(|x| y * self.width + x),
                    (x + 1 < self.width).then_some(i + 1),
                    y.checked_sub(1).map(|y| y * self.width + x),
                    (y + 1 < self.height).then_some(i + self.width),
                ]
                .into_iter()
                .flatten()
                {
                    let n = neighbor as usize;
                    if !visited[n] {
                        visited[n] = true;
                        if open(n) {
                            region.push(neighbor);
                        }
                    }
                }
            }
            if region.len() < (options.minimum_area as usize)
                || edge && !options.include_open_regions
            {
                continue;
            }
            let color = if hint_weight > 0 {
                hint_sum.map(|v| (v / hint_weight) as u8)
            } else {
                options.palette[((first as u64 * 2654435761)
                    % options.palette.len() as u64)
                    as usize]
            };
            for i in &region {
                let i = *i as usize;
                output[i * 4..i * 4 + 4].copy_from_slice(&[
                    color[0],
                    color[1],
                    color[2],
                    self.selection_value(i),
                ]);
                changed = true;
            }
        }
        if !changed {
            return false;
        }
        let mut next = self.clone();
        let count = next.layers.len();
        next.add_layer();
        if next.layers.len() != count + 1 {
            return false;
        }
        next.layers[next.active].name = "자동 채색".into();
        next.layers[next.active].pixels = Arc::new(output);
        if !next.validate() {
            return false;
        }
        *self = next;
        true
    }
    pub fn match_layer_colors(
        &mut self,
        index: usize,
        reference: &PixelClipboard,
    ) -> bool {
        if !reference.validate()
            || self.pixel_edit_reason(index, PixelEdit::Invert).is_some()
        {
            return false;
        }
        let Some((source_mean, source_std)) =
            statistics(&self.layers[index].pixels, |i| self.selection_value(i))
        else {
            return false;
        };
        let Some((target_mean, target_std)) =
            statistics(&reference.pixels, |_| 255)
        else {
            return false;
        };
        self.edit_layer_pixels(
            index,
            PixelEdit::Statistics {
                source_mean,
                source_std,
                target_mean,
                target_std,
            },
        )
    }
    pub fn match_layer_colors_with(
        &mut self,
        index: usize,
        reference: &PixelClipboard,
        strength: f32,
        preserve: bool,
    ) -> bool {
        if !strength.is_finite()
            || !(0. ..=1.).contains(&strength)
            || strength == 0.
        {
            return false;
        }
        if strength == 1. && !preserve {
            return self.match_layer_colors(index, reference);
        }
        let before = self.clone();
        let mut next = self.clone();
        if !next.match_layer_colors(index, reference) {
            return false;
        }
        let mut pixels = next.layers[index].pixels.as_ref().clone();
        for (i, p) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let original: &[u8] =
                &before.layers[index].pixels[i * 4..i * 4 + 4];
            let original_rgb = [original[0], original[1], original[2]]
                .map(|v| f32::from(v) / 255.);
            let mut rgb = [p[0], p[1], p[2]].map(|v| f32::from(v) / 255.);
            if preserve {
                rgb = crate::preserve_luminance(
                    rgb,
                    original_rgb[0] * 0.2126
                        + original_rgb[1] * 0.7152
                        + original_rgb[2] * 0.0722,
                );
            }
            for c in 0..3 {
                p[c] = ((original_rgb[c] * (1. - strength) + rgb[c] * strength)
                    .clamp(0., 1.)
                    * 255.)
                    .round() as u8;
            }
        }
        if pixels == *before.layers[index].pixels {
            return false;
        }
        next.layers[index].pixels = Arc::new(pixels);
        *self = next;
        true
    }
    pub fn shade_layer(
        &mut self,
        index: usize,
        angle: f32,
        strength: f32,
    ) -> bool {
        if !angle.is_finite()
            || !strength.is_finite()
            || !(0. ..=1.).contains(&strength)
            || self.pixel_edit_reason(index, PixelEdit::Invert).is_some()
            || strength == 0.
        {
            return false;
        }
        let source = &self.layers[index].pixels;
        let mut output = source.as_ref().clone();
        let (sin, cos) = angle.to_radians().sin_cos();
        let light = [cos * 0.7, sin * 0.7, 0.71414286];
        let height = |x: u32, y: u32| {
            let i = (y * self.width + x) as usize * 4;
            let p = &source[i..i + 4];
            (f32::from(p[0]) * 0.2126
                + f32::from(p[1]) * 0.7152
                + f32::from(p[2]) * 0.0722)
                * f32::from(p[3])
                / (255. * 255.)
        };
        for y in 0..self.height {
            for x in 0..self.width {
                let i = (y * self.width + x) as usize;
                let before: [u8; 4] =
                    source[i * 4..i * 4 + 4].try_into().unwrap();
                if before[3] == 0 {
                    continue;
                }
                let dx = (height((x + 1).min(self.width - 1), y)
                    - height(x.saturating_sub(1), y))
                    * 4.;
                let dy = (height(x, (y + 1).min(self.height - 1))
                    - height(x, y.saturating_sub(1)))
                    * 4.;
                let norm = (dx * dx + dy * dy + 1.).sqrt();
                let diffuse = ((-dx * light[0] - dy * light[1] + light[2])
                    / norm)
                    .clamp(0., 1.);
                let factor = 1. - strength * (1. - diffuse);
                let mut after = before;
                for c in 0..3 {
                    after[c] = (f32::from(before[c]) * factor).round() as u8;
                }
                output[i * 4..i * 4 + 4].copy_from_slice(
                    &crate::selection::interpolate_pixel(
                        before,
                        after,
                        self.selection_value(i),
                    ),
                );
            }
        }
        if output == **source {
            return false;
        }
        self.layers[index].pixels = Arc::new(output);
        self.touch();
        true
    }
}
fn statistics(
    pixels: &[u8],
    coverage: impl Fn(usize) -> u8,
) -> Option<([f32; 3], [f32; 3])> {
    let mut weight = 0.;
    let mut sums = [0.; 3];
    let mut squares = [0.; 3];
    for (i, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
        let w = f64::from(pixel[3]) * f64::from(coverage(i));
        weight += w;
        for c in 0..3 {
            let value = f64::from(pixel[c]) / 255.;
            sums[c] += value * w;
            squares[c] += value * value * w;
        }
    }
    if weight == 0. {
        return None;
    }
    Some((
        sums.map(|v| (v / weight) as f32),
        std::array::from_fn(|c| {
            (squares[c] / weight - (sums[c] / weight).powi(2))
                .max(0.)
                .sqrt()
                .max(0.001) as f32
        }),
    ))
}
