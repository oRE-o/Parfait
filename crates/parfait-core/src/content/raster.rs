use super::*;
use std::fmt::Write;
use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Stroke,
    Transform,
};

fn transform(m: Affine) -> Transform {
    let [a, b, c, d, x, y] = m.0;
    Transform::from_row(a, b, c, d, x, y)
}
fn rect(path: &Path) -> ContentBox {
    let r = path.compute_tight_bounds().unwrap_or(path.bounds());
    ContentBox { min: [r.left(), r.top()], max: [r.right(), r.bottom()] }
}
fn union(target: &mut Option<ContentBox>, value: ContentBox) {
    *target = Some(target.map_or(value, |b| b.union(value)));
}
struct Outline {
    builder: PathBuilder,
    count: usize,
    overflow: bool,
}
impl Outline {
    fn accept(&mut self, n: usize) -> bool {
        self.count += n;
        if self.count > MAX_CONTENT_POINTS {
            self.overflow = true;
        }
        !self.overflow
    }
}
impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.accept(1) {
            self.builder.move_to(x, y);
        }
    }
    fn line_to(&mut self, x: f32, y: f32) {
        if self.accept(1) {
            self.builder.line_to(x, y);
        }
    }
    fn quad_to(&mut self, a: f32, b: f32, x: f32, y: f32) {
        if self.accept(2) {
            self.builder.quad_to(a, b, x, y);
        }
    }
    fn curve_to(&mut self, a: f32, b: f32, c: f32, d: f32, x: f32, y: f32) {
        if self.accept(3) {
            self.builder.cubic_to(a, b, c, d, x, y);
        }
    }
    fn close(&mut self) {
        if self.accept(1) {
            self.builder.close();
        }
    }
}
struct Geometry {
    paths: Vec<(Path, [u8; 4], Option<f32>, Affine)>,
    layout: Option<ContentBox>,
    ink: Option<ContentBox>,
    center: Option<ContentBox>,
    wire: u64,
    logical: u64,
}
fn geometry(body: &LayerContent) -> Result<Geometry, &'static str> {
    let matrix = body.matrix();
    if !matrix.validate() {
        return Err("변환 행렬이 유효하지 않습니다.");
    }
    let mut result = Geometry {
        paths: Vec::new(),
        layout: None,
        ink: None,
        center: None,
        wire: 0,
        logical: 0,
    };
    match body {
        LayerContent::Vector(v) => {
            if v.paths.len() > MAX_CONTENT_PATHS {
                return Err("벡터 경로는 256개 이하여야 합니다.");
            }
            let mut count = 0;
            result.wire = 36;
            result.logical = std::mem::size_of::<LayerContent>() as u64
                + v.paths.capacity() as u64
                    * std::mem::size_of::<VectorPath>() as u64;
            for p in &v.paths {
                count += p.points.len();
                if count > MAX_CONTENT_POINTS
                    || p.points.len() < 2
                    || !p.width.is_finite()
                    || !(0.1..=512.).contains(&p.width)
                    || p.points
                        .iter()
                        .flatten()
                        .any(|v| !v.is_finite() || v.abs() > 16384.)
                {
                    return Err("경로의 점·선 두께·좌표 범위를 확인해 주세요.");
                }
                result.wire += 18 + p.points.len() as u64 * 8;
                result.logical += p.points.capacity() as u64 * 8;
                let mut builder = PathBuilder::new();
                builder.move_to(p.points[0][0], p.points[0][1]);
                for point in &p.points[1..] {
                    builder.line_to(point[0], point[1]);
                }
                if p.closed {
                    builder.close();
                }
                let path =
                    builder.finish().ok_or("벡터 경로를 만들 수 없습니다.")?;
                let center =
                    ContentBox::points(&p.points).unwrap().transformed(matrix);
                union(&mut result.center, center);
                let mut ink = ContentBox::points(&p.points).unwrap();
                if !p.fill {
                    for c in 0..2 {
                        ink.min[c] -= p.width * 0.5;
                        ink.max[c] += p.width * 0.5;
                    }
                }
                union(&mut result.ink, ink.transformed(matrix));
                result.paths.push((
                    path,
                    p.color,
                    (!p.fill).then_some(p.width),
                    matrix,
                ));
            }
            result.layout = result.ink;
        }
        LayerContent::Text(t) => {
            if t.font.len() > MAX_CONTENT_FONT_BYTES
                || t.text.len() > 1024
                || t.text.chars().count() > 128
                || t.text
                    .chars()
                    .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
                || !t.size.is_finite()
                || !(8. ..=512.).contains(&t.size)
            {
                return Err(
                    "텍스트는 128자, 글꼴은 16 MiB, 크기는 8~512 이하여야 합니다.",
                );
            }
            let face = ttf_parser::Face::parse(&t.font, 0)
                .map_err(|_| "읽을 수 없는 글꼴입니다.")?;
            let scale = t.size / f32::from(face.units_per_em());
            let asc = f32::from(face.ascender()) * scale;
            let line = (f32::from(face.ascender())
                - f32::from(face.descender())
                + f32::from(face.line_gap()))
            .max(f32::from(face.units_per_em()))
                * scale;
            let mut x = 0.;
            let mut y = asc;
            let mut max_x = 0_f32;
            let mut count = 0;
            for c in t.text.chars() {
                if c == '\n' {
                    max_x = max_x.max(x);
                    x = 0.;
                    y += line;
                    continue;
                }
                if c == '\t' {
                    x += t.size * 2.;
                    continue;
                }
                let id = face.glyph_index(c).unwrap_or(ttf_parser::GlyphId(0));
                let mut outline = Outline {
                    builder: PathBuilder::new(),
                    count: 0,
                    overflow: false,
                };
                let bbox = face.outline_glyph(id, &mut outline);
                count += outline.count;
                if outline.overflow || count > MAX_CONTENT_POINTS {
                    return Err("글자 윤곽이 편집 한도를 초과합니다.");
                }
                if bbox.is_some()
                    && let Some(path) = outline.builder.finish()
                {
                    let local = Affine([scale, 0., 0., -scale, x, y]);
                    let mapping = local.then(matrix);
                    let actual = path
                        .clone()
                        .transform(transform(mapping))
                        .ok_or("글자 변환 범위를 초과합니다.")?;
                    union(&mut result.ink, rect(&actual));
                    result.paths.push((path, t.color, None, mapping));
                }
                x += f32::from(
                    face.glyph_hor_advance(id).unwrap_or(face.units_per_em()),
                ) * scale;
            }
            max_x = max_x.max(x);
            if !t.text.is_empty() {
                result.layout = Some(
                    ContentBox { min: [0., 0.], max: [max_x, y - asc + line] }
                        .transformed(matrix),
                );
            }
            result.wire = 52 + t.text.len() as u64 + t.font.len() as u64;
            result.logical = std::mem::size_of::<LayerContent>() as u64
                + t.text.capacity() as u64
                + t.font.capacity() as u64;
        }
    }
    if [result.layout, result.ink, result.center]
        .into_iter()
        .flatten()
        .any(|b| !b.validate())
    {
        return Err("내용의 변환 범위를 초과합니다.");
    }
    Ok(result)
}
pub(super) fn statistics(
    body: &LayerContent,
) -> Result<ContentStats, &'static str> {
    let g = geometry(body)?;
    Ok(ContentStats {
        wire: g.wire,
        logical: g.logical.max(g.wire)
            + std::mem::size_of::<ContentNode>() as u64,
        layout: g.layout,
        ink: g.ink,
        center: g.center,
    })
}
pub(super) fn render(
    body: &LayerContent,
    width: u32,
    height: u32,
) -> Result<Arc<Vec<u8>>, &'static str> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err("캔버스 크기를 확인해 주세요.");
    }
    let g = geometry(body)?;
    // Bound repeated scan conversion work before allocating a canvas. Path point
    // count and source coordinates are separately limited by geometry().
    let work = g
        .paths
        .iter()
        .try_fold(0_u64, |sum, (path, _, stroke, m)| {
            let mut b = rect(path);
            if let Some(w) = stroke {
                for c in 0..2 {
                    b.min[c] -= *w * 0.5;
                    b.max[c] += *w * 0.5;
                }
            }
            let b = b.transformed(*m);
            let w = (b.max[0].min(width as f32) - b.min[0].max(0.))
                .max(0.)
                .ceil() as u64;
            let h = (b.max[1].min(height as f32) - b.min[1].max(0.))
                .max(0.)
                .ceil() as u64;
            sum.checked_add(w * h + path.points().len() as u64 * 64)
        })
        .ok_or("렌더링 작업 한도를 초과합니다.")?;
    if work > 128 * 1024 * 1024 {
        return Err(
            "내용을 나누거나 캔버스 크기를 줄여 주세요 (렌더링 작업 한도).",
        );
    }
    let mut pixmap = Pixmap::new(width, height)
        .ok_or("픽셀 메모리를 확보하지 못했습니다.")?;
    for (path, color, stroke, m) in g.paths {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        paint.anti_alias = true;
        if let Some(width) = stroke {
            pixmap.stroke_path(
                &path,
                &paint,
                &Stroke {
                    width,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Default::default()
                },
                transform(m),
                None,
            );
        } else {
            pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                transform(m),
                None,
            );
        }
    }
    let mut bytes = pixmap.take();
    for p in bytes.as_chunks_mut::<4>().0 {
        let a = u32::from(p[3]);
        for v in &mut p[..3] {
            *v = (u32::from(*v) * 255 + a / 2)
                .checked_div(a)
                .unwrap_or(0)
                .min(255) as u8;
        }
    }
    Ok(Arc::new(bytes))
}
pub(super) fn svg(
    body: &LayerContent,
    width: u32,
    height: u32,
) -> Result<String, &'static str> {
    let g = geometry(body)?;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">"
    );
    for (path, color, stroke, m) in g.paths {
        let [a, b, c, d, x, y] = m.0;
        write!(out, "<path transform=\"matrix({a} {b} {c} {d} {x} {y})\" d=\"")
            .unwrap();
        for s in path.segments() {
            use tiny_skia::PathSegment::*;
            match s {
                MoveTo(p) => write!(out, "M{} {} ", p.x, p.y),
                LineTo(p) => write!(out, "L{} {} ", p.x, p.y),
                QuadTo(p, q) => {
                    write!(out, "Q{} {} {} {} ", p.x, p.y, q.x, q.y)
                }
                CubicTo(p, q, r) => write!(
                    out,
                    "C{} {} {} {} {} {} ",
                    p.x, p.y, q.x, q.y, r.x, r.y
                ),
                Close => write!(out, "Z "),
            }
            .unwrap();
        }
        let [r, g, b, alpha] = color;
        let opacity = f32::from(alpha) / 255.;
        if let Some(width) = stroke {
            write!(out,"\" fill=\"none\" stroke=\"rgb({r},{g},{b})\" stroke-opacity=\"{opacity}\" stroke-width=\"{width}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>").unwrap();
        } else {
            write!(
                out,
                "\" fill=\"rgb({r},{g},{b})\" fill-opacity=\"{opacity}\"/>"
            )
            .unwrap();
        }
        if out.len() > 16 * 1024 * 1024 {
            return Err("SVG가 16 MiB를 초과합니다.");
        }
    }
    out.push_str("</svg>");
    Ok(out)
}
