use crate::{Document, PixelRect, composite_over};
use std::sync::Arc;

pub(crate) enum Destination<'a> {
    Shared(&'a mut Arc<Vec<u8>>),
    Unique(&'a mut Vec<u8>),
}
impl<'a> Destination<'a> {
    pub fn write(self, index: usize, result: [u8; 4]) -> (Self, bool) {
        match self {
            Self::Shared(pixels) => {
                if pixels[index..index + 4] == result {
                    return (Self::Shared(pixels), false);
                }
                let pixels = Arc::make_mut(pixels);
                pixels[index..index + 4].copy_from_slice(&result);
                (Self::Unique(pixels), true)
            }
            Self::Unique(pixels) => {
                let changed = pixels[index..index + 4] != result;
                if changed {
                    pixels[index..index + 4].copy_from_slice(&result);
                }
                (Self::Unique(pixels), changed)
            }
        }
    }
    #[inline]
    pub(crate) fn pixel(&self, index: usize) -> [u8; 4] {
        match self {
            Self::Shared(pixels) => {
                pixels[index..index + 4].try_into().unwrap()
            }
            Self::Unique(pixels) => {
                pixels[index..index + 4].try_into().unwrap()
            }
        }
    }

    #[inline]
    pub(crate) fn edit(
        self,
        index: usize,
        change: impl FnOnce(&mut [u8]) -> bool,
    ) -> (Self, bool) {
        match self {
            Self::Shared(pixels) => {
                let mut result: [u8; 4] =
                    pixels[index..index + 4].try_into().unwrap();
                if !change(&mut result) {
                    return (Self::Shared(pixels), false);
                }
                let pixels = Arc::make_mut(pixels);
                pixels[index..index + 4].copy_from_slice(&result);
                (Self::Unique(pixels), true)
            }
            Self::Unique(pixels) => {
                let changed = change(&mut pixels[index..index + 4]);
                (Self::Unique(pixels), changed)
            }
        }
    }
}

pub(crate) fn recolor_locked(
    pixel: &mut [u8],
    color: [u8; 3],
    amount: f32,
) -> bool {
    if pixel[3] == 0 || !amount.is_finite() || amount <= 0. {
        return false;
    }
    let amount = amount.min(1.);
    let mut changed = false;
    for (target, source) in pixel[..3].iter_mut().zip(color) {
        let next = (f32::from(*target) * (1. - amount)
            + f32::from(source) * amount)
            .round() as u8;
        changed |= *target != next;
        *target = next;
    }
    changed
}

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
    Image { width: u32, height: u32, mask: Arc<Vec<u8>> },
}
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum GapMode {
    #[default]
    Fixed,
    Wide,
    Normal,
    Narrow,
}
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum BrushOperation {
    #[default]
    Paint,
    Erase,
    Blend,
    Blur,
    Smudge,
}
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum OpacityMode {
    #[default]
    PerDab,
    PerStroke,
}
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Brush {
    pub size: f32,
    pub opacity: f32,
    pub opacity_mode: OpacityMode,
    pub color: [u8; 3],
    pub erase: bool,
    pub operation: BrushOperation,
    pub color_stretch: f32,
    pub blur_automatic: bool,
    pub blur_radius: f32,
    pub pressure: bool,
    pub tip: BrushTip,
    pub pressure_curve: crate::ResponseCurve,
    pub minimum_size: f32,
    pub pressure_opacity: bool,
    pub flow: f32,
    pub spacing: f32,
    pub pressure_spacing: bool,
    pub minimum_spacing: f32,
    pub spacing_curve: crate::ResponseCurve,
    pub gap_mode: GapMode,
    pub continuous_spray: bool,
    pub spray_rate: f32,
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
            opacity_mode: OpacityMode::PerDab,
            color: [46, 43, 54],
            erase: false,
            operation: BrushOperation::Paint,
            color_stretch: 0.75,
            blur_automatic: true,
            blur_radius: 2.,
            pressure: true,
            tip: BrushTip::Round,
            pressure_curve: Default::default(),
            minimum_size: 0.05,
            pressure_opacity: false,
            flow: 1.,
            spacing: 0.12,
            pressure_spacing: false,
            minimum_spacing: 0.1,
            spacing_curve: Default::default(),
            gap_mode: GapMode::Fixed,
            continuous_spray: false,
            spray_rate: 30.,
            hardness: 1.,
            aspect: 1.,
            angle: 0.,
            smoothing: 0.,
        }
    }
}
impl Brush {
    pub fn operation(&self) -> BrushOperation {
        if self.erase { BrushOperation::Erase } else { self.operation }
    }
    pub fn blur_radius_pixels(&self, radius: f32) -> usize {
        if self.blur_automatic {
            (radius * 0.1).ceil()
        } else {
            self.blur_radius.round()
        }
        .clamp(1., 32.) as usize
    }
    pub fn spacing_pixels(&self) -> f32 {
        let ratio = match self.gap_mode {
            GapMode::Fixed => self.spacing,
            GapMode::Wide => 0.5,
            GapMode::Normal => 0.25,
            GapMode::Narrow => 0.1,
        };
        (self.size * ratio).max(0.5)
    }
    pub fn mapped_pressure(&self, raw: f32) -> f32 {
        let p = if raw.is_finite() { raw.clamp(0., 1.) } else { 1. };
        self.pressure_curve.sample(p)
    }
    pub fn spacing_pixels_at(&self, pressure: f32) -> f32 {
        if self.gap_mode != GapMode::Fixed || !self.pressure_spacing {
            return self.spacing_pixels();
        }
        let pressure =
            if pressure.is_finite() { pressure.clamp(0., 1.) } else { 1. };
        let factor = self.minimum_spacing
            + (1. - self.minimum_spacing) * self.spacing_curve.sample(pressure);
        (self.size * self.spacing * factor).max(0.5)
    }
    pub fn validate(&self) -> bool {
        let bounded = |v: f32, a, b| v.is_finite() && (a..=b).contains(&v);
        bounded(self.size, 1., 512.)
            && bounded(self.opacity, 0., 1.)
            && bounded(self.flow, 0.01, 1.)
            && bounded(self.spacing, 0.01, 10.)
            && bounded(self.minimum_spacing, 0., 1.)
            && self.spacing_curve.validate()
            && bounded(self.spray_rate, 1., 120.)
            && bounded(self.hardness, 0., 1.)
            && bounded(self.aspect, 0.1, 1.)
            && bounded(self.angle, -180., 180.)
            && bounded(self.smoothing, 0., 0.95)
            && bounded(self.minimum_size, 0., 1.)
            && bounded(self.color_stretch, 0., 1.)
            && bounded(self.blur_radius, 1., 32.)
            && self.pressure_curve.validate()
            && match &self.tip {
                BrushTip::Image { width, height, mask } => {
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
    layer_id: crate::LayerId,
    alpha_locked: bool,
    damage: Option<PixelRect>,
    last: Point,
    spacing_left: f32,
    brush: Brush,
    spray_remainder: f64,
    carry: Option<[f32; 4]>,
    scratch: crate::raster_brush::Scratch,
}
impl Stroke {
    pub fn begin(doc: &mut Document, brush: Brush, point: Point) -> Self {
        let scratch = crate::raster_brush::Scratch::new(&brush, doc);
        Self::begin_with_scratch(doc, brush, point, scratch)
    }
    pub fn begin_mask(
        doc: &mut Document,
        brush: Brush,
        point: Point,
    ) -> Option<Self> {
        if !doc.validate()
            || !brush.validate()
            || !matches!(
                brush.operation(),
                BrushOperation::Paint | BrushOperation::Erase
            )
            || !doc.layers[doc.active]
                .mask
                .as_ref()
                .is_some_and(|mask| mask.enabled)
        {
            return None;
        }
        if !doc.layer_ancestors_allow_edit(doc.active) {
            return None;
        }
        let scratch = crate::raster_brush::Scratch::Mask(
            crate::layer_mask::paint::MaskPaint::new(doc, &brush),
        );
        Some(Self::begin_with_scratch(doc, brush, point, scratch))
    }
    fn begin_with_scratch(
        doc: &mut Document,
        brush: Brush,
        point: Point,
        scratch: crate::raster_brush::Scratch,
    ) -> Self {
        let mut stroke = Self {
            layer_id: doc.layers[doc.active].id,
            alpha_locked: doc.layers[doc.active].alpha_locked,
            damage: None,
            last: point,
            spacing_left: 0.,
            brush,
            spray_remainder: 0.,
            carry: None,
            scratch,
        };
        stroke.damage = stroke.stamp(doc, point);
        stroke
    }
    fn stamp(&mut self, doc: &mut Document, point: Point) -> Option<PixelRect> {
        if doc
            .layers
            .get(doc.active)
            .is_none_or(|layer| layer.id != self.layer_id)
            || doc.layers[doc.active].alpha_locked != self.alpha_locked
        {
            return None;
        }
        if let crate::raster_brush::Scratch::Mask(mask) = &mut self.scratch {
            return mask.stamp(doc, &self.brush, point);
        }
        match self.brush.operation() {
            BrushOperation::Paint | BrushOperation::Erase => {
                if let crate::raster_brush::Scratch::Wash(wash) =
                    &mut self.scratch
                {
                    wash.stamp(doc, &self.brush, point)
                } else {
                    stamp(doc, &self.brush, point)
                }
            }
            BrushOperation::Blend => crate::raster_brush::blend(
                doc,
                &self.brush,
                point,
                &mut self.carry,
            ),
            BrushOperation::Blur => crate::raster_brush::blur(
                doc,
                &self.brush,
                point,
                &mut self.scratch,
            ),
            BrushOperation::Smudge => {
                if let crate::raster_brush::Scratch::Smudge(patch) =
                    &mut self.scratch
                {
                    crate::smudge::stamp(doc, &self.brush, point, patch)
                } else {
                    None
                }
            }
        }
    }
    pub fn scratch_capacity_bytes(&self) -> usize {
        self.scratch.capacity_bytes()
    }
    /// Drain all dab bounds since the last renderer update.
    pub fn take_damage(&mut self) -> Option<PixelRect> {
        self.damage.take()
    }
    pub fn spray_interval(&self) -> Option<f32> {
        (self.brush.continuous_spray
            && self.brush.operation() != BrushOperation::Smudge)
            .then_some(1. / self.brush.spray_rate)
    }
    pub fn spray(&mut self, doc: &mut Document, elapsed: f32, pressure: f32) {
        if self.brush.operation() == BrushOperation::Smudge
            || !self.brush.continuous_spray
            || !elapsed.is_finite()
            || elapsed <= 0.
        {
            return;
        }
        let interval = 1. / f64::from(self.brush.spray_rate);
        let elapsed = self.spray_remainder + f64::from(elapsed.min(0.1));
        let count = ((elapsed + 0.0000001) / interval).floor() as usize;
        self.spray_remainder = (elapsed - count as f64 * interval).max(0.);
        self.last.pressure = pressure;
        for _ in 0..count.min(12) {
            if let Some(region) = self.stamp(doc, self.last) {
                self.damage =
                    Some(self.damage.map_or(region, |old| old.union(region)));
            }
        }
    }
    pub fn extend(&mut self, doc: &mut Document, point: Point) {
        let point = Point {
            x: self.last.x
                + (point.x - self.last.x) * (1. - self.brush.smoothing),
            y: self.last.y
                + (point.y - self.last.y) * (1. - self.brush.smoothing),
            ..point
        };
        let dx = point.x - self.last.x;
        let dy = point.y - self.last.y;
        let dist = dx.hypot(dy);
        if !dist.is_finite() {
            return;
        }
        if dist == 0. {
            self.last = point;
            return;
        }
        let spacing = self.brush.spacing_pixels_at(self.last.pressure);
        let mut step =
            if self.spacing_left == 0. { spacing } else { self.spacing_left };
        while step <= dist {
            let t = step / dist;
            let pressure =
                self.last.pressure + (point.pressure - self.last.pressure) * t;
            let damage = self.stamp(
                doc,
                Point {
                    x: self.last.x + dx * t,
                    y: self.last.y + dy * t,
                    pressure,
                },
            );
            if let Some(r) = damage {
                self.damage = Some(self.damage.map_or(r, |old| old.union(r)));
            }
            step += if self.brush.pressure_spacing
                && self.brush.gap_mode == GapMode::Fixed
            {
                self.brush.spacing_pixels_at(pressure)
            } else {
                spacing
            };
        }
        self.spacing_left = step - dist;
        self.last = point;
    }
}
pub(crate) struct Dab {
    pub bounds: PixelRect,
    pub radius: f32,
    mapped: f32,
    center: Point,
    sin: f32,
    cos: f32,
}
impl Dab {
    pub fn new(doc: &Document, brush: &Brush, point: Point) -> Option<Self> {
        Self::new_target(doc, brush, point, false)
    }
    pub(crate) fn new_mask(
        doc: &Document,
        brush: &Brush,
        point: Point,
    ) -> Option<Self> {
        Self::new_target(doc, brush, point, true)
    }
    fn new_target(
        doc: &Document,
        brush: &Brush,
        point: Point,
        mask: bool,
    ) -> Option<Self> {
        if matches!(doc.selection, Some(crate::MaskCoverage::Uniform(0))) {
            return None;
        }
        if !point.x.is_finite() || !point.y.is_finite() {
            return None;
        }
        let layer = &doc.layers[doc.active];
        if (!mask
            && (layer.kind != crate::LayerKind::Raster
                || doc.native_content(doc.active).is_some()))
            || !doc.layer_ancestors_allow_edit(doc.active)
        {
            return None;
        }
        let mapped = brush.mapped_pressure(point.pressure);
        let pressure = if brush.pressure {
            brush.minimum_size + (1. - brush.minimum_size) * mapped
        } else {
            1.
        };
        let radius = (brush.size * pressure / 2.).clamp(0.5, 256.);
        let x0 = ((point.x - radius - 1.).floor() as i32).max(0);
        let y0 = ((point.y - radius - 1.).floor() as i32).max(0);
        let x1 = ((point.x + radius + 1.).ceil() as i32).min(doc.width as i32);
        let y1 = ((point.y + radius + 1.).ceil() as i32).min(doc.height as i32);
        if x0 >= x1 || y0 >= y1 {
            return None;
        }
        let (sin, cos) = brush.angle.to_radians().sin_cos();
        Some(Self {
            bounds: PixelRect {
                x0: x0 as u32,
                y0: y0 as u32,
                x1: x1 as u32,
                y1: y1 as u32,
            },
            radius,
            mapped,
            center: point,
            sin,
            cos,
        })
    }
    pub fn coverage(&self, brush: &Brush, column: u32, row: u32) -> f32 {
        let dx = (column as f32 + 0.5 - self.center.x) / self.radius;
        let dy = (row as f32 + 0.5 - self.center.y) / self.radius;
        let nx = self.cos * dx + self.sin * dy;
        let ny = (-self.sin * dx + self.cos * dy) / brush.aspect;
        match &brush.tip {
            BrushTip::Round => ((1. - nx.hypot(ny))
                / (1. - brush.hardness).max(1. / self.radius))
            .clamp(0., 1.),
            BrushTip::Soft => (1. - nx.hypot(ny)).clamp(0., 1.).powi(2),
            BrushTip::Pencil => {
                ((1. - nx.hypot(ny)) * self.radius).clamp(0., 1.)
                    * (0.25
                        + (((column as i32).wrapping_mul(374761393)
                            ^ (row as i32).wrapping_mul(668265263))
                            as u32
                            % 100) as f32
                            / 133.)
            }
            BrushTip::Flat => {
                ((1. - nx.abs().max(ny.abs())) * self.radius).clamp(0., 1.)
            }
            BrushTip::Image { width, height, mask } => {
                if nx.abs() > 1. || ny.abs() > 1. {
                    0.
                } else {
                    let ix = (((nx + 1.) * 0.5 * *width as f32) as u32)
                        .min(width - 1);
                    let iy = (((ny + 1.) * 0.5 * *height as f32) as u32)
                        .min(height - 1);
                    mask[(iy * width + ix) as usize] as f32 / 255.
                }
            }
        }
    }
    pub fn alpha(&self, brush: &Brush, column: u32, row: u32) -> f32 {
        self.coverage(brush, column, row)
            * brush.opacity
            * brush.flow
            * if brush.pressure_opacity { self.mapped } else { 1. }
    }
    pub fn has_strength(&self, brush: &Brush) -> bool {
        brush.opacity > 0.
            && brush.flow > 0.
            && (!brush.pressure_opacity || self.mapped > 0.)
    }
    pub fn opacity_limit(&self, brush: &Brush) -> f32 {
        brush.opacity * if brush.pressure_opacity { self.mapped } else { 1. }
    }
}
fn stamp(doc: &mut Document, brush: &Brush, point: Point) -> Option<PixelRect> {
    let dab = Dab::new(doc, brush, point)?;
    if !dab.has_strength(brush) {
        return None;
    }
    let alpha_locked = doc.layers[doc.active].alpha_locked;
    let mut destination =
        Destination::Shared(&mut doc.layers[doc.active].pixels);
    let mut changed = false;
    for row in dab.bounds.y0..dab.bounds.y1 {
        for column in dab.bounds.x0..dab.bounds.x1 {
            let alpha = crate::selection::strength(
                doc.selection.as_ref(),
                (row * doc.width + column) as usize,
                dab.alpha(brush, column, row),
            );
            if alpha <= 0. {
                continue;
            }
            let idx = ((row * doc.width + column) * 4) as usize;
            let mut result = destination.pixel(idx);
            if alpha_locked {
                recolor_locked(
                    &mut result,
                    if brush.operation() == BrushOperation::Erase {
                        [255; 3]
                    } else {
                        brush.color
                    },
                    alpha,
                );
            } else if brush.operation() == BrushOperation::Erase {
                result[3] = (result[3] as f32 * (1. - alpha)).round() as u8;
            } else {
                composite_over(&mut result, brush.color, alpha);
            }
            let (next_destination, pixel_changed) =
                destination.write(idx, result);
            destination = next_destination;
            changed |= pixel_changed;
        }
    }
    if changed {
        doc.touch();
        Some(dab.bounds)
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ineffective_dabs_keep_shared_pixels_and_revision_in_both_accumulation_modes()
     {
        for opacity_mode in [OpacityMode::PerDab, OpacityMode::PerStroke] {
            for case in 0..4 {
                let mut document = Document::new(32, 32);
                let mut brush = Brush {
                    opacity_mode,
                    pressure: false,
                    continuous_spray: true,
                    ..Default::default()
                };
                match case {
                    0 => brush.opacity = 0.,
                    1 => {
                        brush.tip = BrushTip::Image {
                            width: 1,
                            height: 1,
                            mask: Arc::new(vec![0]),
                        }
                    }
                    2 => Arc::make_mut(&mut document.layers[1].pixels)
                        .as_chunks_mut::<4>()
                        .0
                        .fill([
                            brush.color[0],
                            brush.color[1],
                            brush.color[2],
                            255,
                        ]),
                    _ => brush.operation = BrushOperation::Erase,
                }
                let original = document.clone();
                let mut stroke = Stroke::begin(
                    &mut document,
                    brush,
                    Point { x: 16.5, y: 16.5, pressure: 1. },
                );
                stroke.spray(&mut document, 0.1, 1.);
                assert!(stroke.take_damage().is_none());
                assert_eq!(document.revision, original.revision);
                assert!(Arc::ptr_eq(
                    &document.layers[1].pixels,
                    &original.layers[1].pixels
                ));
                assert_eq!(stroke.scratch_capacity_bytes(), 0);
            }
        }
    }
    #[test]
    fn spacing_pressure_is_independent_bounded_and_optional() {
        let mut brush = Brush {
            size: 20.,
            spacing: 2.,
            pressure_spacing: true,
            minimum_spacing: 0.25,
            ..Default::default()
        };
        assert_eq!(brush.spacing_pixels_at(0.), 10.);
        assert_eq!(brush.spacing_pixels_at(0.5), 25.);
        assert_eq!(brush.spacing_pixels_at(1.), 40.);
        assert_eq!(brush.spacing_pixels_at(f32::NAN), 40.);
        brush.pressure_curve = [1., 1., 1., 1., 1.].into();
        brush.pressure = false;
        assert_eq!(brush.spacing_pixels_at(0.), 10.);
        brush.spacing_curve = [1., 0.75, 0.5, 0.25, 0.].into();
        assert_eq!(brush.spacing_pixels_at(1.), 10.);
        assert_eq!(brush.spacing_pixels_at(0.), 40.);
        brush.minimum_spacing = 0.;
        assert_eq!(brush.spacing_pixels_at(1.), 0.5);
        for mode in [GapMode::Wide, GapMode::Normal, GapMode::Narrow] {
            brush.gap_mode = mode;
            assert_eq!(brush.spacing_pixels_at(0.), brush.spacing_pixels());
            assert_eq!(brush.spacing_pixels_at(1.), brush.spacing_pixels());
        }
        brush.minimum_spacing = -0.1;
        assert!(!brush.validate());
        brush.minimum_spacing = f32::NAN;
        assert!(!brush.validate());
        brush.minimum_spacing = 0.25;
        brush.spacing_curve.points[0][0] = 0.2;
        assert!(!brush.validate());
    }

    #[test]
    fn pressure_gap_changes_real_dabs_without_changing_diameter() {
        let brush = Brush {
            size: 4.,
            spacing: 4.,
            pressure: false,
            pressure_spacing: true,
            minimum_spacing: 0.25,
            ..Default::default()
        };
        let render = |pressure| {
            let mut doc = Document::new(96, 32);
            let mut stroke = Stroke::begin(
                &mut doc,
                brush.clone(),
                Point { x: 16., y: 16., pressure },
            );
            stroke.extend(&mut doc, Point { x: 80., y: 16., pressure });
            assert_eq!(stroke.scratch_capacity_bytes(), 0);
            doc
        };
        let dense = render(0.);
        let sparse = render(1.);
        let alpha = |doc: &Document, column: usize| {
            doc.layers[1].pixels[(16 * 96 + column) * 4 + 3]
        };
        assert!(alpha(&dense, 20) > 0);
        assert_eq!(alpha(&sparse, 20), 0);
        assert_eq!(alpha(&dense, 16), alpha(&sparse, 16));
    }

    #[test]
    fn pressure_gap_retains_distance_across_event_boundaries() {
        let brush = Brush {
            size: 16.,
            spacing: 0.5,
            pressure: false,
            flow: 0.4,
            pressure_spacing: true,
            minimum_spacing: 0.125,
            ..Default::default()
        };
        let render = |pieces| {
            let mut doc = Document::new(192, 48);
            let point = |step: usize| Point {
                x: 16. + 128. * step as f32 / pieces as f32,
                y: 24.,
                pressure: 0.125 + 0.75 * step as f32 / pieces as f32,
            };
            let mut stroke = Stroke::begin(&mut doc, brush.clone(), point(0));
            for step in 1..=pieces {
                stroke.extend(&mut doc, point(step));
            }
            (doc, stroke.spacing_left)
        };
        let (single, remainder) = render(1);
        for pieces in [2, 8, 64, 256] {
            let (split, split_remainder) = render(pieces);
            assert!((remainder - split_remainder).abs() < 0.0002);
            for (single, split) in single.layers[1]
                .pixels
                .iter()
                .zip(split.layers[1].pixels.iter())
            {
                assert!(single.abs_diff(*split) <= 1);
            }
        }
    }

    #[test]
    fn disabled_pressure_gap_and_automatic_modes_keep_legacy_pixels() {
        for mode in
            [GapMode::Fixed, GapMode::Wide, GapMode::Normal, GapMode::Narrow]
        {
            let mut brush = Brush {
                size: 20.,
                gap_mode: mode,
                pressure_spacing: false,
                ..Default::default()
            };
            let render = |brush| {
                let mut doc = Document::new(96, 48);
                let mut stroke = Stroke::begin(
                    &mut doc,
                    brush,
                    Point { x: 8., y: 24., pressure: 0.2 },
                );
                stroke
                    .extend(&mut doc, Point { x: 80., y: 24., pressure: 0.8 });
                doc
            };
            let original = render(brush.clone());
            brush.minimum_spacing = 0.8;
            brush.spacing_curve = [1., 0.9, 0.4, 0.2, 0.].into();
            assert_eq!(
                original.layers[1].pixels,
                render(brush.clone()).layers[1].pixels
            );
            if mode != GapMode::Fixed {
                brush.pressure_spacing = true;
                assert_eq!(
                    original.layers[1].pixels,
                    render(brush).layers[1].pixels
                );
            }
        }
    }

    #[test]
    fn tangent_handles_change_actual_paint_without_moving_pressure_anchors() {
        let mut automatic = Document::new(64, 64);
        let mut bent = automatic.clone();
        let mut brush =
            Brush { size: 40., pressure_opacity: true, ..Default::default() };
        let point = Point { x: 32., y: 32., pressure: 0.3 };
        Stroke::begin(&mut automatic, brush.clone(), point);
        let anchors = brush.pressure_curve.points.clone();
        brush.pressure_curve.controls = Some(vec![[[0.12, 0.9], [0.7, 0.95]]]);
        assert!(brush.validate());
        assert_eq!(brush.pressure_curve.points, anchors);
        Stroke::begin(&mut bent, brush, point);
        let alpha = |document: &Document| {
            document.layers[1]
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|pixel| u64::from(pixel[3]))
                .sum::<u64>()
        };
        assert!(alpha(&bent) > alpha(&automatic) * 3);
    }
    #[test]
    fn arbitrary_control_point_pressure_changes_actual_dab_pixels() {
        let mut low = Document::new(64, 64);
        let mut high = low.clone();
        let brush = Brush {
            size: 40.,
            pressure_curve: crate::ResponseCurve {
                points: vec![[0., 0.], [0.17, 0.9], [0.62, 0.1], [1., 1.]],
                controls: None,
            },
            pressure_opacity: true,
            ..Default::default()
        };
        assert!(brush.validate());
        let point = Point { x: 32., y: 32., pressure: 0.62 };
        Stroke::begin(&mut low, brush.clone(), point);
        Stroke::begin(&mut high, brush, Point { pressure: 0.17, ..point });
        let total_alpha = |document: &Document| {
            document.layers[1]
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|pixel| u64::from(pixel[3]))
                .sum::<u64>()
        };
        assert!(total_alpha(&high) > total_alpha(&low) * 10);
    }
    #[test]
    fn pressure_curve_controls_coverage_and_opacity() {
        let mut low = Document::new(64, 64);
        let mut high = low.clone();
        let b = Brush {
            size: 40.,
            pressure_curve: [0., 0.06, 0.25, 0.56, 1.].into(),
            pressure_opacity: true,
            ..Default::default()
        };
        assert!(b.validate());
        assert_eq!(b.mapped_pressure(0.5), 0.25);
        Stroke::begin(
            &mut low,
            b.clone(),
            Point { x: 32., y: 32., pressure: 0.5 },
        );
        Stroke::begin(&mut high, b, Point { x: 32., y: 32., pressure: 1. });
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
            pressure_curve: [0., 0.8, f32::NAN, 0.9, 1.].into(),
            ..Default::default()
        };
        assert!(!invalid.validate());
    }
    #[test]
    fn continuous_stroke_and_erase() {
        let mut d = Document::new(64, 32);
        let b = Brush { size: 8., ..Default::default() };
        let mut s = Stroke::begin(
            &mut d,
            b.clone(),
            Point { x: 4., y: 16., pressure: 1. },
        );
        s.extend(&mut d, Point { x: 60., y: 16., pressure: 1. });
        for x in 4..60 {
            assert!(d.layers[1].pixels[(16 * 64 + x) * 4 + 3] > 0);
        }
        let mut eraser = b;
        eraser.erase = true;
        Stroke::begin(&mut d, eraser, Point { x: 32., y: 16., pressure: 1. });
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
            Point { x: 4., y: 4., pressure: 1. },
        );
        assert_eq!(d, before);
    }
}
