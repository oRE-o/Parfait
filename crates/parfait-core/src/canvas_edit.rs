//! Real document geometry edits, including masks and selection coverage.
use crate::{Document, LayerKind, MaskCoverage, PixelRect};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CanvasEdit {
    Resize { width: u32, height: u32 },
    ResizeNearest { width: u32, height: u32 },
    SuperResolve { width: u32, height: u32, iterations: u8, strength: f32 },
    Extent { width: u32, height: u32, offset: [i32; 2] },
    Crop(PixelRect),
    RotateRight,
    RotateHalf,
    RotateLeft,
    FlipHorizontal,
    FlipVertical,
}
impl CanvasEdit {
    pub fn dimensions(self, doc: &Document) -> (u32, u32) {
        match self {
            Self::Resize { width, height }
            | Self::ResizeNearest { width, height }
            | Self::SuperResolve { width, height, .. }
            | Self::Extent { width, height, .. } => (width, height),
            Self::Crop(r) => {
                (r.x1.saturating_sub(r.x0), r.y1.saturating_sub(r.y0))
            }
            Self::RotateLeft | Self::RotateRight => (doc.height, doc.width),
            _ => (doc.width, doc.height),
        }
    }
    fn source(self, x: u32, y: u32, doc: &Document) -> (f32, f32) {
        let (x, y) = (x as f32, y as f32);
        match self {
            Self::Resize { width, height }
            | Self::ResizeNearest { width, height }
            | Self::SuperResolve { width, height, .. } => (
                (x + 0.5) * doc.width as f32 / width as f32 - 0.5,
                (y + 0.5) * doc.height as f32 / height as f32 - 0.5,
            ),
            Self::Extent { offset, .. } => {
                (x - offset[0] as f32, y - offset[1] as f32)
            }
            Self::Crop(r) => (x + r.x0 as f32, y + r.y0 as f32),
            Self::RotateRight => (y, doc.height as f32 - 1. - x),
            Self::RotateHalf => {
                (doc.width as f32 - 1. - x, doc.height as f32 - 1. - y)
            }
            Self::RotateLeft => (doc.width as f32 - 1. - y, x),
            Self::FlipHorizontal => (doc.width as f32 - 1. - x, y),
            Self::FlipVertical => (x, doc.height as f32 - 1. - y),
        }
    }
}
impl Document {
    /// Conservative allocation estimate; uniform coverage can become dense.
    pub fn canvas_edit_bytes(&self, edit: CanvasEdit) -> Option<u64> {
        let (w, h) = edit.dimensions(self);
        if w == 0 || h == 0 || w > 4096 || h > 4096 {
            return None;
        }
        let channels = self
            .layers
            .iter()
            .map(|l| {
                4 * u64::from(l.kind == LayerKind::Raster)
                    + u64::from(l.mask.is_some())
            })
            .sum::<u64>()
            + u64::from(self.selection.is_some());
        let bytes = u64::from(w) * u64::from(h) * channels
            + self.contents.iter().map(|n| n.logical_bytes()).sum::<u64>();
        (bytes <= 512 * 1024 * 1024).then_some(bytes)
    }
    pub fn canvas_edit_reason(&self, edit: CanvasEdit) -> Option<&'static str> {
        if !self.validate() {
            return Some("이미지 데이터가 유효하지 않습니다.");
        }
        if self.canvas_edit_bytes(edit).is_none() {
            return Some(
                "크기는 1~4096픽셀이며 전체 픽셀 예산은 512 MiB 이하여야 합니다.",
            );
        }
        if matches!(edit, CanvasEdit::SuperResolve { .. })
            && !self.contents.is_empty()
        {
            return Some(
                "초해상도 처리를 하려면 텍스트·벡터를 먼저 래스터화해 주세요.",
            );
        }
        if let CanvasEdit::SuperResolve { width, height, iterations, strength } =
            edit
            && (width < self.width
                || height < self.height
                || (width == self.width && height == self.height)
                || !(1..=8).contains(&iterations)
                || !strength.is_finite()
                || !(0. ..=1.).contains(&strength))
        {
            return Some(
                "출력 크기는 원본 이상이어야 하며 반복 횟수는 1~8회, 보정 강도는 0~1이어야 합니다.",
            );
        }
        if let CanvasEdit::Crop(r) = edit
            && (r.x0 >= r.x1
                || r.y0 >= r.y1
                || r.x1 > self.width
                || r.y1 > self.height)
        {
            return Some("잘라낼 범위가 캔버스 밖에 있습니다.");
        }
        None
    }
    pub fn edit_canvas(&mut self, edit: CanvasEdit) -> bool {
        if self.canvas_edit_reason(edit).is_some() {
            return false;
        }
        let (width, height) = edit.dimensions(self);
        if matches!(
            edit,
            CanvasEdit::Resize { .. }
                | CanvasEdit::ResizeNearest { .. }
                | CanvasEdit::Extent { offset: [0, 0], .. }
        ) && (width, height) == (self.width, self.height)
        {
            return false;
        }
        let mut next = self.clone();
        next.width = width;
        next.height = height;
        for (index, layer) in next.layers.iter_mut().enumerate() {
            let source = &self.layers[index];
            if layer.kind == LayerKind::Raster {
                let mut pixels = vec![0; (width * height * 4) as usize];
                for y in 0..height {
                    for x in 0..width {
                        let (sx, sy) = edit.source(x, y, self);
                        let pixel = if matches!(
                            edit,
                            CanvasEdit::Resize { .. }
                                | CanvasEdit::SuperResolve { .. }
                        ) {
                            sample_rgba(
                                &source.pixels,
                                self.width,
                                self.height,
                                sx,
                                sy,
                            )
                        } else if matches!(
                            edit,
                            CanvasEdit::ResizeNearest { .. }
                        ) {
                            let sx =
                                sx.round().clamp(0., self.width as f32 - 1.)
                                    as u32;
                            let sy =
                                sy.round().clamp(0., self.height as f32 - 1.)
                                    as u32;
                            let i = ((sy * self.width + sx) * 4) as usize;
                            source.pixels[i..i + 4].try_into().unwrap()
                        } else if sx >= 0.
                            && sy >= 0.
                            && sx < self.width as f32
                            && sy < self.height as f32
                        {
                            let i = (sy as u32 * self.width + sx as u32)
                                as usize
                                * 4;
                            source.pixels[i..i + 4].try_into().unwrap()
                        } else {
                            [0; 4]
                        };
                        let i = (y * width + x) as usize * 4;
                        pixels[i..i + 4].copy_from_slice(&pixel);
                    }
                }
                if let CanvasEdit::SuperResolve {
                    iterations, strength, ..
                } = edit
                {
                    back_project(
                        &mut pixels,
                        [width, height],
                        &source.pixels,
                        [self.width, self.height],
                        iterations,
                        strength,
                    );
                }
                if pixels != *source.pixels {
                    layer.pixels = Arc::new(pixels);
                }
            }
            if let Some(mask) = &mut layer.mask {
                let previous = &source.mask.as_ref().unwrap().coverage;
                let coverage = remap_coverage(previous, self, edit, 255);
                if coverage != *previous {
                    mask.coverage = coverage;
                }
            }
        }
        if let Some(previous) = &self.selection {
            let coverage = remap_coverage(previous, self, edit, 0);
            if coverage != *previous {
                next.selection = Some(coverage);
            }
        }
        if !self.contents.is_empty() {
            use crate::Affine;
            let matrix = match edit {
                CanvasEdit::Resize { width, height }
                | CanvasEdit::ResizeNearest { width, height } => Affine([
                    width as f32 / self.width as f32,
                    0.,
                    0.,
                    height as f32 / self.height as f32,
                    0.,
                    0.,
                ]),
                CanvasEdit::Extent { offset, .. } => {
                    Affine::translation(offset[0] as f32, offset[1] as f32)
                }
                CanvasEdit::Crop(r) => {
                    Affine::translation(-(r.x0 as f32), -(r.y0 as f32))
                }
                CanvasEdit::RotateRight => {
                    Affine([0., 1., -1., 0., self.height as f32, 0.])
                }
                CanvasEdit::RotateLeft => {
                    Affine([0., -1., 1., 0., 0., self.width as f32])
                }
                CanvasEdit::RotateHalf => Affine([
                    -1.,
                    0.,
                    0.,
                    -1.,
                    self.width as f32,
                    self.height as f32,
                ]),
                CanvasEdit::FlipHorizontal => {
                    Affine([-1., 0., 0., 1., self.width as f32, 0.])
                }
                CanvasEdit::FlipVertical => {
                    Affine([1., 0., 0., -1., 0., self.height as f32])
                }
                CanvasEdit::SuperResolve { .. } => return false,
            };
            for node in &mut next.contents {
                let Ok(transformed) = node.transformed(matrix) else {
                    return false;
                };
                let Ok(pixels) = transformed.render(width, height) else {
                    return false;
                };
                let index = next
                    .layers
                    .iter()
                    .position(|l| l.id == node.layer)
                    .unwrap();
                next.layers[index].pixels = pixels;
                *node = transformed;
            }
        }
        if !next.validate() || next.same_edit_state(self) {
            return false;
        }
        next.touch();
        *self = next;
        true
    }
}
fn remap_coverage(
    coverage: &MaskCoverage,
    doc: &Document,
    edit: CanvasEdit,
    outside: u8,
) -> MaskCoverage {
    let (w, h) = edit.dimensions(doc);
    let mut data = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = edit.source(x, y, doc);
            let (sx, sy) = if matches!(
                edit,
                CanvasEdit::Resize { .. }
                    | CanvasEdit::ResizeNearest { .. }
                    | CanvasEdit::SuperResolve { .. }
            ) {
                (
                    sx.round().clamp(0., doc.width as f32 - 1.),
                    sy.round().clamp(0., doc.height as f32 - 1.),
                )
            } else {
                (sx, sy)
            };
            data.push(
                if sx >= 0.
                    && sy >= 0.
                    && sx < doc.width as f32
                    && sy < doc.height as f32
                {
                    coverage.value((sy as u32 * doc.width + sx as u32) as usize)
                } else {
                    outside
                },
            );
        }
    }
    let first = data[0];
    if data.iter().all(|v| *v == first) {
        MaskCoverage::Uniform(first)
    } else {
        MaskCoverage::Dense(Arc::new(data))
    }
}
fn sample_rgba(
    pixels: &[u8],
    width: u32,
    height: u32,
    x: f32,
    y: f32,
) -> [u8; 4] {
    let x = x.clamp(0., width as f32 - 1.);
    let y = y.clamp(0., height as f32 - 1.);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let mut total = [0.; 4];
    for (sx, sy, weight) in [
        (x0, y0, (1. - fx) * (1. - fy)),
        ((x0 + 1).min(width - 1), y0, fx * (1. - fy)),
        (x0, (y0 + 1).min(height - 1), (1. - fx) * fy),
        ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1), fx * fy),
    ] {
        let i = (sy * width + sx) as usize * 4;
        let alpha = f32::from(pixels[i + 3]) / 255.;
        for c in 0..3 {
            total[c] += f32::from(pixels[i + c]) * alpha * weight;
        }
        total[3] += alpha * weight;
    }
    if total[3] == 0. {
        return [0; 4];
    }
    [
        (total[0] / total[3]).round() as u8,
        (total[1] / total[3]).round() as u8,
        (total[2] / total[3]).round() as u8,
        (total[3] * 255.).round() as u8,
    ]
}

// Iterative back projection enforces consistency with the low-resolution input.
// This deterministic reconstruction does not use learned detail synthesis.
fn back_project(
    high: &mut [u8],
    size: [u32; 2],
    low: &[u8],
    original: [u32; 2],
    iterations: u8,
    strength: f32,
) {
    if strength == 0. {
        return;
    }
    let [w, h] = size;
    let [ow, oh] = original;
    let mut residual = vec![[0.; 3]; (ow * oh) as usize];
    for _ in 0..iterations {
        for y in 0..oh {
            for x in 0..ow {
                let predicted = sample_rgba(
                    high,
                    w,
                    h,
                    (x as f32 + 0.5) * w as f32 / ow as f32 - 0.5,
                    (y as f32 + 0.5) * h as f32 / oh as f32 - 0.5,
                );
                let i = (y * ow + x) as usize;
                let weight = f32::from(low[i * 4 + 3]) / 255.;
                for c in 0..3 {
                    residual[i][c] = (f32::from(low[i * 4 + c])
                        - f32::from(predicted[c]))
                        * weight;
                }
            }
        }
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize * 4;
                if high[i + 3] == 0 {
                    continue;
                }
                let sx = ((x as f32 + 0.5) * ow as f32 / w as f32 - 0.5)
                    .clamp(0., ow as f32 - 1.);
                let sy = ((y as f32 + 0.5) * oh as f32 / h as f32 - 0.5)
                    .clamp(0., oh as f32 - 1.);
                let x0 = sx.floor() as u32;
                let y0 = sy.floor() as u32;
                let fx = sx - x0 as f32;
                let fy = sy - y0 as f32;
                let mut correction = [0.; 3];
                for (px, py, weight) in [
                    (x0, y0, (1. - fx) * (1. - fy)),
                    ((x0 + 1).min(ow - 1), y0, fx * (1. - fy)),
                    (x0, (y0 + 1).min(oh - 1), (1. - fx) * fy),
                    ((x0 + 1).min(ow - 1), (y0 + 1).min(oh - 1), fx * fy),
                ] {
                    for c in 0..3 {
                        correction[c] +=
                            residual[(py * ow + px) as usize][c] * weight;
                    }
                }
                for c in 0..3 {
                    high[i + c] =
                        (f32::from(high[i + c]) + correction[c] * strength)
                            .round()
                            .clamp(0., 255.) as u8;
                }
            }
        }
    }
}
