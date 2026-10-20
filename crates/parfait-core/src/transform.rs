use crate::{Document, LayerKind, MaskCoverage, PixelRect};
use std::sync::Arc;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WarpKind {
    #[default]
    Projective,
    Mesh,
    Pins,
}
/// Transient puppet control in normalized content coordinates. Rotation is
/// clockwise in document (Y-down) degrees; no model or network is involved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WarpPin {
    pub source: [f32; 2],
    pub target: [f32; 2],
    pub rotation: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RasterTransform {
    pub kind: WarpKind,
    pub scale: [f32; 2],
    pub angle: f32,
    pub shear: [f32; 2],
    pub translation: [f32; 2],
    pub perspective: [f32; 2],
    /// Offsets of grid points, relative to content width and height.
    pub nodes: Vec<[f32; 2]>,
    pub grid: [u8; 2],
    pub pins: Vec<WarpPin>,
}
impl Default for RasterTransform {
    fn default() -> Self {
        Self {
            kind: WarpKind::Projective,
            scale: [1., 1.],
            angle: 0.,
            shear: [0.; 2],
            translation: [0.; 2],
            perspective: [0.; 2],
            nodes: vec![[0.; 2]; 9],
            grid: [3, 3],
            pins: Vec::new(),
        }
    }
}
mod mask;
mod mesh;
use mesh::PreparedMesh;
impl RasterTransform {
    pub fn validate(&self) -> bool {
        self.scale
            .iter()
            .all(|v| v.is_finite() && (0.01..=100.).contains(&v.abs()))
            && self.angle.is_finite()
            && self.angle.abs() <= 3600.
            && self.shear.iter().all(|v| v.is_finite() && v.abs() <= 4.)
            && self
                .translation
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 8192.)
            && self.perspective.iter().all(|v| v.is_finite() && v.abs() <= 0.95)
            && self
                .nodes
                .iter()
                .flatten()
                .all(|v| v.is_finite() && v.abs() <= 2.)
            && self.grid.iter().all(|n| (2..=16).contains(n))
            && self.nodes.len()
                == usize::from(self.grid[0]) * usize::from(self.grid[1])
            && self.pins.len() <= 32
            && self.pins.iter().enumerate().all(|(i, p)| {
                p.source.iter().all(|v| v.is_finite() && (0. ..=1.).contains(v))
                    && p.target
                        .iter()
                        .all(|v| v.is_finite() && (-2. ..=3.).contains(v))
                    && p.rotation.is_finite()
                    && p.rotation.abs() <= 3600.
                    && self.pins[..i]
                        .iter()
                        .all(|q| distance(p.source, q.source) > 1e-8)
            })
            && if self.kind == WarpKind::Projective {
                (self.scale[0] * self.scale[1] - self.shear[0] * self.shear[1])
                    .abs()
                    > 0.0001
            } else {
                PreparedMesh::new(self).is_some()
            }
    }
    pub fn unmap_point(
        &self,
        point: [f32; 2],
        bounds: PixelRect,
    ) -> Option<[f32; 2]> {
        if !self.validate() {
            return None;
        }
        let mesh = if self.kind != WarpKind::Projective {
            PreparedMesh::new(self)
        } else {
            None
        };
        self.inverse(point[0], point[1], bounds, mesh.as_ref())
    }
    pub fn fit_quad(bounds: PixelRect, quad: [[f32; 2]; 4]) -> Option<Self> {
        let w = (bounds.x1 - bounds.x0) as f32;
        let h = (bounds.y1 - bounds.y0) as f32;
        if w <= 0. || h <= 0. || quad.iter().flatten().any(|v| !v.is_finite()) {
            return None;
        }
        let cx = bounds.x0 as f32 + w * 0.5;
        let cy = bounds.y0 as f32 + h * 0.5;
        let source = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];
        let mut matrix = [[0_f64; 9]; 8];
        for i in 0..4 {
            let [x, y] = source[i];
            let u = f64::from((quad[i][0] - cx) / w);
            let v = f64::from((quad[i][1] - cy) / h);
            matrix[i * 2] = [x, y, 1., 0., 0., 0., -u * x, -u * y, u];
            matrix[i * 2 + 1] = [0., 0., 0., x, y, 1., -v * x, -v * y, v];
        }
        for c in 0..8 {
            let pivot = (c..8).max_by(|a, b| {
                matrix[*a][c].abs().total_cmp(&matrix[*b][c].abs())
            })?;
            if matrix[pivot][c].abs() < 1e-12 {
                return None;
            }
            matrix.swap(c, pivot);
            let divisor = matrix[c][c];
            for v in &mut matrix[c][c..] {
                *v /= divisor;
            }
            for row in 0..8 {
                if row == c {
                    continue;
                }
                let factor = matrix[row][c];
                let pivot = matrix[c];
                for (value, pivot) in
                    matrix[row][c..].iter_mut().zip(&pivot[c..])
                {
                    *value -= factor * pivot;
                }
            }
        }
        let [a, b, tx, c, d, ty, p, q] =
            std::array::from_fn(|i| matrix[i][8] as f32);
        let result = Self {
            scale: [a - tx * p, d - ty * q],
            shear: [(b - tx * q) * w / h, (c - ty * p) * h / w],
            translation: [tx * w, ty * h],
            perspective: [p, q],
            ..Default::default()
        };
        result.validate().then_some(result)
    }
    pub fn resize_grid(&mut self, columns: u8, rows: u8) -> bool {
        if !(2..=16).contains(&columns) || !(2..=16).contains(&rows) {
            return false;
        }
        self.grid = [columns, rows];
        self.nodes = vec![[0.; 2]; usize::from(columns) * usize::from(rows)];
        true
    }
    pub fn mesh_source(&self, i: usize) -> [f32; 2] {
        [
            (i % usize::from(self.grid[0])) as f32
                / f32::from(self.grid[0] - 1),
            (i / usize::from(self.grid[0])) as f32
                / f32::from(self.grid[1] - 1),
        ]
    }
    pub fn map_point(
        &self,
        point: [f32; 2],
        bounds: PixelRect,
    ) -> Option<[f32; 2]> {
        if self.kind != WarpKind::Projective {
            return None;
        }
        let width = (bounds.x1 - bounds.x0) as f32;
        let height = (bounds.y1 - bounds.y0) as f32;
        let center =
            [bounds.x0 as f32 + width * 0.5, bounds.y0 as f32 + height * 0.5];
        let x = point[0] - center[0];
        let y = point[1] - center[1];
        let denom = 1.
            + self.perspective[0] * x / width
            + self.perspective[1] * y / height;
        if denom.abs() < 1e-6 {
            return None;
        }
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let u = self.scale[0] * x + self.shear[0] * y;
        let v = self.shear[1] * x + self.scale[1] * y;
        Some([
            center[0] + self.translation[0] + (cos * u - sin * v) / denom,
            center[1] + self.translation[1] + (sin * u + cos * v) / denom,
        ])
    }
    pub fn mesh_triangles(&self) -> Option<Vec<[[f32; 2]; 3]>> {
        Some(PreparedMesh::new(self)?.targets())
    }
    fn inverse(
        &self,
        x: f32,
        y: f32,
        bounds: PixelRect,
        mesh: Option<&PreparedMesh>,
    ) -> Option<[f32; 2]> {
        let width = (bounds.x1 - bounds.x0) as f32;
        let height = (bounds.y1 - bounds.y0) as f32;
        let center =
            [bounds.x0 as f32 + width / 2., bounds.y0 as f32 + height / 2.];
        if self.kind == WarpKind::Projective {
            let (sin, cos) = self.angle.to_radians().sin_cos();
            let [sx, sy] = self.scale;
            let [hx, hy] = self.shear;
            let [dx, dy] = [
                x - center[0] - self.translation[0],
                y - center[1] - self.translation[1],
            ];
            let [p, q] =
                [self.perspective[0] / width, self.perspective[1] / height];
            let a = cos * sx - sin * hy - dx * p;
            let b = cos * hx - sin * sy - dx * q;
            let c = sin * sx + cos * hy - dy * p;
            let d = sin * hx + cos * sy - dy * q;
            let det = a * d - b * c;
            if det.abs() < 1e-6 {
                return None;
            }
            let source = [
                (d * dx - b * dy) / det + center[0],
                (a * dy - c * dx) / det + center[1],
            ];
            return source.iter().all(|v| v.is_finite()).then_some(source);
        }
        let p = mesh?.inverse([
            (x - bounds.x0 as f32) / width,
            (y - bounds.y0 as f32) / height,
        ])?;
        Some([
            bounds.x0 as f32 + p[0] * width,
            bounds.y0 as f32 + p[1] * height,
        ])
    }
    fn identity(&self) -> bool {
        match self.kind {
            WarpKind::Projective => {
                self.scale == [1., 1.]
                    && self.angle == 0.
                    && self.shear == [0.; 2]
                    && self.translation == [0.; 2]
                    && self.perspective == [0.; 2]
            }
            WarpKind::Mesh => self.nodes.iter().all(|p| *p == [0.; 2]),
            WarpKind::Pins => self
                .pins
                .iter()
                .all(|p| p.source == p.target && p.rotation == 0.),
        }
    }
}
fn sub(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn cross(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[1] - a[1] * b[0]
}
fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}
fn barycentric(p: [f32; 2], t: [[f32; 2]; 3]) -> Option<[f32; 3]> {
    let b = sub(t[1], t[0]);
    let c = sub(t[2], t[0]);
    let p = sub(p, t[0]);
    let det = cross(b, c);
    if det.abs() < 1e-9 {
        return None;
    }
    let v = cross(p, c) / det;
    let w = cross(b, p) / det;
    let u = 1. - v - w;
    (u >= -0.00001 && v >= -0.00001 && w >= -0.00001).then_some([u, v, w])
}
impl Document {
    pub fn transform_reason(
        &self,
        index: usize,
        transform: &RasterTransform,
    ) -> Option<&'static str> {
        if !self.validate() || !transform.validate() {
            return Some("변형 값이 유효하지 않거나 메시가 뒤집혀 있습니다.");
        }
        let Some(layer) = self.layers.get(index) else {
            return Some("변형할 레이어가 없습니다.");
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
        if layer.alpha_locked {
            return Some("투명 픽셀 잠금을 해제한 뒤 변형해 주세요.");
        }
        if self.selection.is_some() && layer.mask.is_some() {
            return Some(
                "선택 범위 변형은 마스크가 없는 레이어에서 사용할 수 있습니다. 선택을 해제하면 마스크도 함께 변형됩니다.",
            );
        }
        if self.transform_bounds(index).is_none() {
            return Some("변형할 픽셀 또는 선택 범위가 없습니다.");
        }
        None
    }
    pub fn transform_bounds(&self, index: usize) -> Option<PixelRect> {
        if self.selection.is_some() {
            self.selection_bounds()
        } else {
            crate::image_edit::bounds(self.width, self.height, |i| {
                self.layers[index].pixels[i * 4 + 3] > 0
            })
        }
    }
    pub fn transform_layer_pixels(
        &mut self,
        index: usize,
        transform: &RasterTransform,
    ) -> bool {
        self.transform_layer_pixels_cancellable(index, transform, || false)
    }
    pub fn transform_layer_pixels_cancellable(
        &mut self,
        index: usize,
        transform: &RasterTransform,
        cancelled: impl Fn() -> bool,
    ) -> bool {
        if cancelled()
            || self.transform_reason(index, transform).is_some()
            || transform.identity()
        {
            return false;
        }
        let bounds = self.transform_bounds(index).unwrap();
        let mesh = if transform.kind != WarpKind::Projective {
            PreparedMesh::new(transform)
        } else {
            None
        };
        let layer = &self.layers[index];
        let source = if self.selection.is_some() {
            let mut pixels = layer.pixels.as_ref().clone();
            for (i, p) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                p[3] = ((u16::from(p[3]) * u16::from(self.selection_value(i))
                    + 127)
                    / 255) as u8;
            }
            Arc::new(pixels)
        } else {
            layer.pixels.clone()
        };
        let mut output = if self.selection.is_some() {
            let mut pixels = layer.pixels.as_ref().clone();
            for (i, p) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                *p = crate::selection::interpolate_pixel(
                    *p,
                    [0; 4],
                    self.selection_value(i),
                );
            }
            pixels
        } else {
            vec![0; layer.pixels.len()]
        };
        let mut selection = self
            .selection
            .as_ref()
            .map(|_| vec![0; (self.width * self.height) as usize]);
        let mut mask = layer
            .mask
            .as_ref()
            .map(|_| vec![255; (self.width * self.height) as usize]);
        for y in 0..self.height {
            if cancelled() {
                return false;
            }
            for x in 0..self.width {
                let i = (y * self.width + x) as usize;
                let Some([sx, sy]) = transform.inverse(
                    x as f32 + 0.5,
                    y as f32 + 0.5,
                    bounds,
                    mesh.as_ref(),
                ) else {
                    continue;
                };
                let pixel = sample(
                    &source,
                    self.width,
                    self.height,
                    sx - 0.5,
                    sy - 0.5,
                );
                let before = output[i * 4..i * 4 + 4].try_into().unwrap();
                output[i * 4..i * 4 + 4].copy_from_slice(&over(before, pixel));
                let inside = sx >= 0.
                    && sy >= 0.
                    && sx < self.width as f32
                    && sy < self.height as f32;
                if let Some(values) = &mut selection
                    && inside
                {
                    values[i] = self.selection_value(
                        sy as usize * self.width as usize + sx as usize,
                    );
                }
                if let Some(values) = &mut mask
                    && inside
                {
                    values[i] =
                        layer.mask.as_ref().unwrap().coverage.value(
                            sy as usize * self.width as usize + sx as usize,
                        );
                }
            }
        }
        let mut next = self.clone();
        if output != *layer.pixels {
            next.layers[index].pixels = Arc::new(output);
        }
        if let Some(values) = mask {
            let coverage = coverage(values);
            if coverage != layer.mask.as_ref().unwrap().coverage {
                next.layers[index].mask.as_mut().unwrap().coverage = coverage;
            }
        }
        if let Some(values) = selection {
            let coverage = coverage(values);
            next.selection = if matches!(coverage, MaskCoverage::Uniform(0)) {
                None
            } else {
                Some(coverage)
            };
        }
        if !next.validate() || next.same_edit_state(self) {
            return false;
        }
        next.touch();
        *self = next;
        true
    }
}
fn coverage(values: Vec<u8>) -> MaskCoverage {
    let first = values[0];
    if values.iter().all(|v| *v == first) {
        MaskCoverage::Uniform(first)
    } else {
        MaskCoverage::Dense(Arc::new(values))
    }
}
fn over(base: [u8; 4], source: [u8; 4]) -> [u8; 4] {
    if source[3] == 0 {
        return base;
    }
    if source[3] == 255 {
        return source;
    }
    let a = f32::from(source[3]) / 255.;
    let b = f32::from(base[3]) / 255. * (1. - a);
    let total = a + b;
    let mut out = [0; 4];
    for c in 0..3 {
        out[c] = ((f32::from(source[c]) * a + f32::from(base[c]) * b) / total)
            .round() as u8;
    }
    out[3] = (total * 255.).round() as u8;
    out
}
fn sample(pixels: &[u8], width: u32, height: u32, x: f32, y: f32) -> [u8; 4] {
    if x < -1. || y < -1. || x >= width as f32 || y >= height as f32 {
        return [0; 4];
    }
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let mut total = [0.; 4];
    for (sx, sy, w) in [
        (x0, y0, (1. - fx) * (1. - fy)),
        (x0 + 1, y0, fx * (1. - fy)),
        (x0, y0 + 1, (1. - fx) * fy),
        (x0 + 1, y0 + 1, fx * fy),
    ] {
        if sx < 0 || sy < 0 || sx >= width as i32 || sy >= height as i32 {
            continue;
        }
        let i = (sy as usize * width as usize + sx as usize) * 4;
        let a = f32::from(pixels[i + 3]) / 255. * w;
        for c in 0..3 {
            total[c] += f32::from(pixels[i + c]) * a;
        }
        total[3] += a;
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
