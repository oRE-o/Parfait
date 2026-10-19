use crate::{
    Brush, Document, PixelRect, Point,
    brush::Dab,
    raster_brush::{integer_premultiplied, interpolate},
};
#[cfg(test)]
use std::sync::Arc;

pub(crate) struct Patch {
    previous: Vec<[u16; 4]>,
    next: Vec<[u16; 4]>,
    side: usize,
    radius: f32,
    center: Option<[f32; 2]>,
}
impl Patch {
    pub fn new(side: usize) -> Self {
        Self {
            previous: Vec::with_capacity(side * side),
            next: Vec::with_capacity(side * side),
            side: 0,
            radius: 1.,
            center: None,
        }
    }
    pub fn capacity_bytes(&self) -> usize {
        (self.previous.capacity() + self.next.capacity()) * 8
    }
    fn advance(&mut self, side: usize, radius: f32, point: Point) {
        std::mem::swap(&mut self.previous, &mut self.next);
        self.side = side;
        self.radius = radius;
        self.center = Some([point.x, point.y]);
    }
}

fn sample(
    width: usize,
    height: usize,
    x: f32,
    y: f32,
    pixel: impl Fn(usize, usize) -> [u16; 4],
) -> [f32; 4] {
    let left = x.floor() as i32;
    let top = y.floor() as i32;
    let read = |column: i32, row: i32| {
        if column < 0 || row < 0 || column as usize >= width || row as usize >= height {
            [0.; 4]
        } else {
            pixel(column as usize, row as usize).map(f32::from)
        }
    };
    let horizontal = x - left as f32;
    interpolate(
        interpolate(read(left, top), read(left + 1, top), horizontal),
        interpolate(read(left, top + 1), read(left + 1, top + 1), horizontal),
        y - top as f32,
    )
}

pub(crate) fn stamp(
    document: &mut Document,
    brush: &Brush,
    point: Point,
    patch: &mut Patch,
) -> Option<PixelRect> {
    let Some(dab) = Dab::new(document, brush, point) else {
        patch.center = None;
        patch.previous.clear();
        return None;
    };
    if brush.color_stretch == 0.
        || !dab.has_strength(brush)
        || patch.center == Some([point.x, point.y])
    {
        return None;
    }
    let side = (dab.radius * 2.).ceil() as usize + 4;
    let had_previous = patch.center.is_some();
    patch.next.clear();
    let source = &document.layers[document.active].pixels;
    let mut any_alpha = false;
    let mut source_alpha = false;
    for row in 0..side {
        for column in 0..side {
            let offset_x = column as f32 - dab.radius - 1.;
            let offset_y = row as f32 - dab.radius - 1.;
            let current = sample(
                document.width as usize,
                document.height as usize,
                point.x + offset_x - 0.5,
                point.y + offset_y - 0.5,
                |column, row| {
                    let index = (row * document.width as usize + column) * 4;
                    integer_premultiplied(source[index..index + 4].try_into().unwrap())
                        .map(|channel| channel as u16)
                },
            );
            let carried = if had_previous && patch.radius == dab.radius {
                patch.previous[row * patch.side + column].map(f32::from)
            } else if had_previous {
                let scale = patch.radius / dab.radius;
                sample(
                    patch.side,
                    patch.side,
                    offset_x * scale + patch.radius + 1.,
                    offset_y * scale + patch.radius + 1.,
                    |column, row| patch.previous[row * patch.side + column],
                )
            } else {
                current
            };
            source_alpha |= current[3] > 0.;
            let mixed = interpolate(current, carried, brush.color_stretch)
                .map(|channel| channel.round().clamp(0., 65025.) as u16);
            any_alpha |= mixed[3] > 0;
            patch.next.push(mixed);
        }
    }
    if !had_previous || (!any_alpha && !source_alpha) {
        patch.advance(side, dab.radius, point);
        return None;
    }
    let alpha_locked = document.layers[document.active].alpha_locked;
    let mut destination =
        crate::brush::Destination::Shared(&mut document.layers[document.active].pixels);
    let mut changed = false;
    for row in dab.bounds.y0..dab.bounds.y1 {
        for column in dab.bounds.x0..dab.bounds.x1 {
            let carried = sample(
                side,
                side,
                column as f32 + 0.5 - point.x + dab.radius + 1.,
                row as f32 + 0.5 - point.y + dab.radius + 1.,
                |column, row| patch.next[row * side + column],
            );
            let color = [
                carried[0] / 255.,
                carried[1] / 255.,
                carried[2] / 255.,
                carried[3] / 65025.,
            ];
            let index = ((row * document.width + column) * 4) as usize;
            let (next, pixel_changed) = destination.edit(index, |pixel| {
                crate::raster_brush::apply_locked(
                    pixel,
                    color,
                    dab.alpha(brush, column, row),
                    alpha_locked,
                )
            });
            destination = next;
            changed |= pixel_changed;
        }
    }
    patch.advance(side, dab.radius, point);
    if changed {
        document.touch();
        Some(dab.bounds)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BrushOperation, BrushTip, History, Stroke};

    fn fixture() -> Document {
        let mut document = Document::new(48, 32);
        for (index, pixel) in Arc::make_mut(&mut document.layers[1].pixels)
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            *pixel = [
                (index % 48 * 5) as u8,
                (index / 48 * 7) as u8,
                (index % 173) as u8,
                255,
            ];
        }
        document
    }
    fn brush() -> Brush {
        Brush {
            operation: BrushOperation::Smudge,
            size: 12.,
            spacing: 1. / 12.,
            color_stretch: 1.,
            pressure: false,
            tip: BrushTip::Flat,
            ..Default::default()
        }
    }
    fn point(x: f32, y: f32) -> Point {
        Point { x, y, pressure: 1. }
    }

    #[test]
    fn spatial_pattern_moves_without_collapsing_into_an_average_or_adding_ink() {
        let original = fixture();
        let draw = |operation, color| {
            let mut document = original.clone();
            let mut stroke = Stroke::begin(
                &mut document,
                Brush {
                    operation,
                    color,
                    ..brush()
                },
                point(8.5, 12.5),
            );
            assert!(document == original);
            assert!(stroke.take_damage().is_none());
            stroke.extend(&mut document, point(20.5, 12.5));
            document
        };
        let result = draw(BrushOperation::Smudge, [255, 0, 255]);
        for row in 10..=14 {
            let destination = (row * 48 + 20) * 4;
            let source = (row * 48 + 8) * 4;
            assert_eq!(
                &result.layers[1].pixels[destination..destination + 4],
                &original.layers[1].pixels[source..source + 4]
            );
        }
        assert!(result == draw(BrushOperation::Smudge, [0, 255, 0]));
        assert!(result.layers != draw(BrushOperation::Blend, [255, 0, 255]).layers);
    }

    #[test]
    fn bilinear_samples_are_alpha_safe_and_clip_to_transparent_outside() {
        let pixels = [[65025, 0, 0, 65025], [0; 4], [0, 0, 65025, 65025], [0; 4]];
        assert_eq!(
            sample(2, 2, 0.5, 0.5, |column, row| pixels[row * 2 + column]),
            [16256.25, 0., 16256.25, 32512.5]
        );
        assert_eq!(
            sample(2, 2, -0.5, 0., |column, row| pixels[row * 2 + column]),
            [32512.5, 0., 0., 32512.5]
        );
        assert_eq!(
            sample(2, 2, -4., 5., |column, row| pixels[row * 2 + column]),
            [0.; 4]
        );
        let mut document = Document::new(20, 12);
        for (index, pixel) in Arc::make_mut(&mut document.layers[1].pixels)
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            *pixel = if index % 20 < 10 {
                [0, 0, 255, 255]
            } else {
                [255, 0, 0, 0]
            };
        }
        let mut stroke = Stroke::begin(&mut document, brush(), point(7.25, 5.75));
        stroke.extend(&mut document, point(15.75, 5.75));
        assert!(
            document.layers[1]
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 0 || pixel[0] == 0)
        );
    }

    #[test]
    fn direction_and_fractional_pressure_changes_preserve_bounds() {
        for end in [
            point(4.25, 7.75),
            point(25.75, 24.25),
            point(-5.25, 15.),
            point(50., 36.),
        ] {
            let mut document = fixture();
            let before = document.clone();
            let mut stroke = Stroke::begin(
                &mut document,
                Brush {
                    pressure: true,
                    pressure_opacity: true,
                    ..brush()
                },
                point(18.25, 15.5),
            );
            stroke.extend(
                &mut document,
                Point {
                    pressure: 0.3,
                    ..end
                },
            );
            if let Some(damage) = stroke.take_damage() {
                assert!(damage.x1 <= document.width && damage.y1 <= document.height);
            }
            assert!(document.validate());
            assert!(document.layers != before.layers);
        }
    }

    #[test]
    fn empty_blocked_zero_strength_and_stationary_strokes_do_not_copy_or_edit_pixels() {
        for blocked in 0..6 {
            let mut document = if blocked == 0 {
                Document::new(48, 32)
            } else {
                fixture()
            };
            document.layers[1].locked = blocked == 1;
            document.layers[1].visible = blocked != 2;
            let before = document.clone();
            let mut stroke = Stroke::begin(
                &mut document,
                Brush {
                    color_stretch: if blocked == 3 { 0. } else { 1. },
                    opacity: if blocked == 4 { 0. } else { 1. },
                    pressure_opacity: blocked == 5,
                    ..brush()
                },
                Point {
                    pressure: if blocked == 5 { 0. } else { 1. },
                    ..point(8.25, 12.5)
                },
            );
            stroke.extend(
                &mut document,
                Point {
                    x: 20.,
                    pressure: if blocked == 5 { 0. } else { 1. },
                    ..point(8.25, 12.5)
                },
            );
            assert!(document == before);
            assert!(Arc::ptr_eq(
                &document.layers[1].pixels,
                &before.layers[1].pixels
            ));
            assert!(stroke.take_damage().is_none());
        }
        let mut document = fixture();
        let before = document.clone();
        let mut stroke = Stroke::begin(
            &mut document,
            Brush {
                continuous_spray: true,
                ..brush()
            },
            point(8.25, 12.5),
        );
        assert!(stroke.spray_interval().is_none());
        for _ in 0..10 {
            stroke.spray(&mut document, 0.1, 0.2);
        }
        assert!(document == before);
        assert!(Arc::ptr_eq(
            &document.layers[1].pixels,
            &before.layers[1].pixels
        ));
    }

    #[test]
    fn invalid_dab_resets_pickup_and_patch_capacity_is_bounded_and_reused() {
        let mut document = fixture();
        let brush = brush();
        let mut patch = Patch::new(16);
        assert!(stamp(&mut document, &brush, point(8.5, 12.5), &mut patch).is_none());
        assert!(stamp(&mut document, &brush, point(-100., -100.), &mut patch).is_none());
        let before = document.clone();
        assert!(stamp(&mut document, &brush, point(20.5, 12.5), &mut patch).is_none());
        assert!(document == before);
        let mut document = Document::new(640, 640);
        let mut stroke = Stroke::begin(
            &mut document,
            Brush {
                size: 512.,
                ..brush
            },
            point(320.25, 320.75),
        );
        assert_eq!(stroke.scratch_capacity_bytes(), 4_260_096);
        stroke.extend(&mut document, point(520.75, 480.25));
        assert_eq!(stroke.scratch_capacity_bytes(), 4_260_096);
    }

    #[test]
    fn event_segmentation_and_spray_do_not_change_spatial_carry_and_history() {
        let original = fixture();
        let mut whole = original.clone();
        let mut segmented = original.clone();
        let brush = Brush {
            continuous_spray: true,
            ..brush()
        };
        let mut first = Stroke::begin(&mut whole, brush.clone(), point(8.5, 12.5));
        let mut second = Stroke::begin(&mut segmented, brush, point(8.5, 12.5));
        let mut history = History::default();
        history.checkpoint(&original);
        first.extend(&mut whole, point(26.5, 12.5));
        for column in 9..=26 {
            second.extend(&mut segmented, point(column as f32 + 0.5, 12.5));
            second.spray(&mut segmented, 0.1, 1.);
        }
        assert!(whole == segmented);
        let after = whole.clone();
        history.undo(&mut whole);
        assert!(whole.layers == original.layers);
        history.redo(&mut whole);
        assert!(whole.layers == after.layers);
    }
}
