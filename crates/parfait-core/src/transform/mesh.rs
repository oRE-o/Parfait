//! Bounded triangulated deformation and a target-space lookup grid. No services.
use super::*;
struct Triangle {
    source: [[f32; 2]; 3],
    target: [[f32; 2]; 3],
}
pub(super) struct PreparedMesh {
    triangles: Vec<Triangle>,
    bins: Vec<Vec<usize>>,
    min: [f32; 2],
    extent: [f32; 2],
}
impl PreparedMesh {
    pub(super) fn new(t: &RasterTransform) -> Option<Self> {
        let mut points = Vec::new();
        let mut indices = Vec::new();
        if t.kind == WarpKind::Mesh {
            if t.grid.iter().any(|n| !(2..=16).contains(n))
                || t.nodes.len()
                    != usize::from(t.grid[0]) * usize::from(t.grid[1])
            {
                return None;
            }
            for i in 0..t.nodes.len() {
                let s = t.mesh_source(i);
                points.push((s, [s[0] + t.nodes[i][0], s[1] + t.nodes[i][1]]));
            }
            let columns = usize::from(t.grid[0]);
            for y in 0..usize::from(t.grid[1] - 1) {
                for x in 0..columns - 1 {
                    let a = y * columns + x;
                    indices.push([a, a + 1, a + columns + 1]);
                    indices.push([a, a + columns + 1, a + columns]);
                }
            }
        } else if t.kind == WarpKind::Pins {
            if t.pins.len() > 32
                || t.pins.iter().any(|p| {
                    p.source.iter().chain(&p.target).any(|v| !v.is_finite())
                        || !p.rotation.is_finite()
                })
            {
                return None;
            }
            // Include control points as mesh vertices so anchored positions are exact.
            for y in 0..=12 {
                for x in 0..=12 {
                    let s = [x as f32 / 12., y as f32 / 12.];
                    points.push((s, rigid_fit(s, &t.pins)));
                }
            }
            for pin in &t.pins {
                if !points.iter().any(|(s, _)| distance(*s, pin.source) < 1e-12)
                {
                    points.push((pin.source, pin.target));
                }
            }
            let input: Vec<_> = points
                .iter()
                .map(|(p, _)| delaunator::Point {
                    x: f64::from(p[0]),
                    y: f64::from(p[1]),
                })
                .collect();
            for triangle in
                delaunator::triangulate(&input).triangles.as_chunks::<3>().0
            {
                let mut tri = *triangle;
                if cross(
                    sub(points[tri[1]].0, points[tri[0]].0),
                    sub(points[tri[2]].0, points[tri[0]].0),
                ) < 0.
                {
                    tri.swap(1, 2);
                }
                indices.push(tri);
            }
        } else {
            return None;
        }
        if points
            .iter()
            .flat_map(|(a, b)| a.iter().chain(b))
            .any(|v| !v.is_finite() || v.abs() > 8.)
        {
            return None;
        }
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        let mut triangles = Vec::new();
        for ids in indices {
            let source = ids.map(|i| points[i].0);
            let target = ids.map(|i| points[i].1);
            if cross(sub(target[1], target[0]), sub(target[2], target[0]))
                <= 1e-9
            {
                return None;
            }
            for p in target {
                for c in 0..2 {
                    min[c] = min[c].min(p[c]);
                    max[c] = max[c].max(p[c]);
                }
            }
            triangles.push(Triangle { source, target });
        }
        if triangles.is_empty() {
            return None;
        }
        let extent = [max[0] - min[0], max[1] - min[1]];
        let mut bins = vec![Vec::new(); 32 * 32];
        for (i, t) in triangles.iter().enumerate() {
            let mut low = [31; 2];
            let mut high = [0; 2];
            for p in t.target {
                for c in 0..2 {
                    let cell = (((p[c] - min[c]) / extent[c] * 32.).floor()
                        as usize)
                        .min(31);
                    low[c] = low[c].min(cell);
                    high[c] = high[c].max(cell);
                }
            }
            for y in low[1]..=high[1] {
                for x in low[0]..=high[0] {
                    bins[y * 32 + x].push(i);
                }
            }
        }
        Some(Self { triangles, bins, min, extent })
    }
    pub(super) fn targets(&self) -> Vec<[[f32; 2]; 3]> {
        self.triangles.iter().map(|t| t.target).collect()
    }
    pub(super) fn inverse(&self, p: [f32; 2]) -> Option<[f32; 2]> {
        let u = (p[0] - self.min[0]) / self.extent[0];
        let v = (p[1] - self.min[1]) / self.extent[1];
        if !(0. ..=1.).contains(&u) || !(0. ..=1.).contains(&v) {
            return None;
        }
        for index in &self.bins
            [((v * 32.) as usize).min(31) * 32 + ((u * 32.) as usize).min(31)]
        {
            let t = &self.triangles[*index];
            if let Some(w) = barycentric(p, t.target) {
                return Some(std::array::from_fn(|c| {
                    t.source.iter().zip(w).map(|(p, w)| p[c] * w).sum()
                }));
            }
        }
        None
    }
}
fn rigid_fit(point: [f32; 2], pins: &[WarpPin]) -> [f32; 2] {
    if pins.is_empty() {
        return point;
    }
    if let Some(pin) = pins.iter().find(|p| distance(point, p.source) < 1e-12) {
        return pin.target;
    }
    let mut total = 0_f64;
    let mut target = [0_f64; 2];
    for pin in pins {
        let w = 1. / f64::from(distance(point, pin.source).max(1e-12));
        let (sin, cos) = f64::from(pin.rotation).to_radians().sin_cos();
        let x = f64::from(point[0] - pin.source[0]);
        let y = f64::from(point[1] - pin.source[1]);
        target[0] += (f64::from(pin.target[0]) + cos * x - sin * y) * w;
        target[1] += (f64::from(pin.target[1]) + sin * x + cos * y) * w;
        total += w;
    }
    [(target[0] / total) as f32, (target[1] / total) as f32]
}
