use crate::{
    Brush, BrushOperation, Document, PixelRect, Point,
    brush::{Dab, Destination, recolor_locked},
    composite_over,
};
use std::sync::Arc;

const SIDE: usize = 64;
const CELLS: usize = SIDE * SIDE;

pub(crate) struct Wash {
    baseline: Arc<Vec<u8>>,
    width: u32,
    height: u32,
    layer: usize,
    columns: usize,
    tiles: Vec<Option<Box<[f32; CELLS]>>>,
    allocated: usize,
}
impl Wash {
    pub fn new(doc: &Document) -> Self {
        Self {
            baseline: doc.layers[doc.active].pixels.clone(),
            width: doc.width,
            height: doc.height,
            layer: doc.active,
            columns: (doc.width as usize).div_ceil(SIDE),
            tiles: Vec::new(),
            allocated: 0,
        }
    }
    pub fn capacity_bytes(&self) -> usize {
        self.tiles.capacity() * std::mem::size_of::<Option<Box<[f32; CELLS]>>>()
            + self.allocated * std::mem::size_of::<[f32; CELLS]>()
    }
    pub fn stamp(&mut self, doc: &mut Document, brush: &Brush, point: Point) -> Option<PixelRect> {
        if doc.width != self.width || doc.height != self.height || doc.active != self.layer {
            return None;
        }
        let dab = Dab::new(doc, brush, point)?;
        if !dab.has_strength(brush) {
            return None;
        }
        let limit = dab.opacity_limit(brush);
        let erase = brush.operation() == BrushOperation::Erase;
        let alpha_locked = doc.layers[self.layer].alpha_locked;
        let color = if alpha_locked && erase {
            [255; 3]
        } else {
            brush.color
        };
        let mut changed = false;
        let mut destination = Destination::Shared(&mut doc.layers[self.layer].pixels);
        for row in dab.bounds.y0 as usize..dab.bounds.y1 as usize {
            for column in dab.bounds.x0 as usize..dab.bounds.x1 as usize {
                let limit = crate::selection::strength(
                    doc.selection.as_ref(),
                    row * self.width as usize + column,
                    limit,
                );
                if limit <= 0. {
                    continue;
                }
                let deposit = dab.coverage(brush, column as u32, row as u32) * brush.flow;
                if deposit <= 0. {
                    continue;
                }
                let index = (row * self.width as usize + column) * 4;
                let mut result: [u8; 4] = self.baseline[index..index + 4].try_into().unwrap();
                if ((erase || alpha_locked) && result[3] == 0)
                    || (alpha_locked && result[..3] == color)
                    || (!erase && result == [color[0], color[1], color[2], 255])
                {
                    continue;
                }
                if self.tiles.is_empty() {
                    self.tiles = vec![None; self.columns * (self.height as usize).div_ceil(SIDE)];
                }
                let tile = self.tiles[row / SIDE * self.columns + column / SIDE]
                    .get_or_insert_with(|| {
                        self.allocated += 1;
                        Box::new([0.; CELLS])
                    });
                let contribution = &mut tile[(row % SIDE) * SIDE + column % SIDE];
                let next = (*contribution + (limit - *contribution).max(0.) * deposit)
                    .min(limit.max(*contribution));
                if next == *contribution {
                    continue;
                }
                *contribution = next;
                if alpha_locked {
                    recolor_locked(&mut result, color, next);
                } else if erase {
                    result[3] = (f32::from(result[3]) * (1. - next)).round() as u8;
                } else {
                    composite_over(&mut result, brush.color, next);
                }
                let (next_destination, pixel_changed) = destination.write(index, result);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BrushTip, OpacityMode, Stroke};

    fn point(pressure: f32) -> Point {
        Point {
            x: 32.5,
            y: 32.5,
            pressure,
        }
    }
    fn brush() -> Brush {
        Brush {
            size: 16.,
            opacity: 0.4,
            flow: 0.25,
            opacity_mode: OpacityMode::PerStroke,
            pressure: false,
            continuous_spray: true,
            spray_rate: 120.,
            ..Default::default()
        }
    }
    fn alpha(doc: &Document) -> u8 {
        doc.layers[1].pixels[(32 * doc.width as usize + 32) * 4 + 3]
    }
    fn hold(doc: &mut Document, stroke: &mut Stroke) {
        for _ in 0..20 {
            stroke.spray(doc, 0.1, 1.);
        }
    }
    #[test]
    fn one_stroke_caps_but_another_stroke_builds_up_and_density_controls_rate() {
        let mut doc = Document::new(128, 96);
        let mut stroke = Stroke::begin(&mut doc, brush(), point(1.));
        assert_eq!(alpha(&doc), 26);
        hold(&mut doc, &mut stroke);
        assert_eq!(alpha(&doc), 102);
        let revision = doc.revision;
        hold(&mut doc, &mut stroke);
        assert_eq!(alpha(&doc), 102);
        assert_eq!(doc.revision, revision);
        drop(stroke);
        let mut stroke = Stroke::begin(&mut doc, brush(), point(1.));
        hold(&mut doc, &mut stroke);
        assert_eq!(alpha(&doc), 163);
        let mut dense = Document::new(128, 96);
        let mut options = brush();
        options.flow = 1.;
        Stroke::begin(&mut dense, options, point(1.));
        assert_eq!(alpha(&dense), 102);
        let mut legacy = Document::new(128, 96);
        let mut options = brush();
        options.opacity_mode = OpacityMode::PerDab;
        let mut stroke = Stroke::begin(&mut legacy, options, point(1.));
        hold(&mut legacy, &mut stroke);
        assert!(alpha(&legacy) > 240);
    }
    #[test]
    fn pressure_raises_target_without_erasing_stronger_contributions() {
        let mut doc = Document::new(128, 96);
        let options = Brush {
            opacity: 0.8,
            flow: 1.,
            pressure_opacity: true,
            ..brush()
        };
        let mut stroke = Stroke::begin(&mut doc, options, point(0.25));
        assert_eq!(alpha(&doc), 51);
        stroke.spray(&mut doc, 0.01, 0.75);
        assert_eq!(alpha(&doc), 153);
        let revision = doc.revision;
        stroke.spray(&mut doc, 0.1, 0.1);
        assert_eq!(alpha(&doc), 153);
        assert_eq!(revision, doc.revision);
    }
    #[test]
    fn erase_caps_against_original_alpha_and_preserves_rgb() {
        let mut doc = Document::new(128, 96);
        Arc::make_mut(&mut doc.layers[1].pixels)
            .as_chunks_mut::<4>()
            .0
            .fill([90, 140, 210, 200]);
        let options = Brush {
            operation: BrushOperation::Erase,
            opacity: 0.25,
            flow: 1.,
            ..brush()
        };
        let mut stroke = Stroke::begin(&mut doc, options.clone(), point(1.));
        hold(&mut doc, &mut stroke);
        assert_eq!(alpha(&doc), 150);
        assert_eq!(
            &doc.layers[1].pixels[(32 * 128 + 32) * 4..(32 * 128 + 32) * 4 + 3],
            &[90, 140, 210]
        );
        drop(stroke);
        Stroke::begin(&mut doc, options, point(1.));
        assert_eq!(alpha(&doc), 113);
    }
    #[test]
    fn paint_recomposites_original_pixels_for_every_tip_and_baseline_alpha() {
        for tip in [
            BrushTip::Round,
            BrushTip::Soft,
            BrushTip::Pencil,
            BrushTip::Flat,
            BrushTip::Image {
                width: 2,
                height: 2,
                mask: Arc::new(vec![0, 80, 180, 255]),
            },
        ] {
            for baseline_alpha in [0, 90, 255] {
                let mut doc = Document::new(128, 96);
                Arc::make_mut(&mut doc.layers[1].pixels)
                    .as_chunks_mut::<4>()
                    .0
                    .fill([200, 60, 20, baseline_alpha]);
                let baseline = doc.layers[1].pixels.clone();
                let options = Brush {
                    tip: tip.clone(),
                    ..brush()
                };
                let dab = Dab::new(&doc, &options, point(1.)).unwrap();
                let mut stroke = Stroke::begin(&mut doc, options.clone(), point(1.));
                for _ in 0..9 {
                    stroke.spray(&mut doc, 1. / 120., 1.);
                }
                for row in dab.bounds.y0..dab.bounds.y1 {
                    for column in dab.bounds.x0..dab.bounds.x1 {
                        let deposit = dab.coverage(&options, column, row) * options.flow;
                        let contribution = options.opacity * (1. - (1. - deposit).powi(10));
                        let index = ((row * doc.width + column) * 4) as usize;
                        let mut expected: [u8; 4] = baseline[index..index + 4].try_into().unwrap();
                        composite_over(&mut expected, options.color, contribution);
                        for (expected, actual) in
                            expected.iter().zip(&doc.layers[1].pixels[index..index + 4])
                        {
                            assert!(expected.abs_diff(*actual) <= 1);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn tiles_are_lazy_reused_and_noop_dabs_do_not_copy_pixels_or_dirty_document() {
        for state in 0..6 {
            let mut doc = Document::new(128, 128);
            let mut options = brush();
            match state {
                0 => options.opacity = 0.,
                1 => doc.layers[1].locked = true,
                2 => doc.layers[1].visible = false,
                3 => {
                    options.tip = BrushTip::Image {
                        width: 1,
                        height: 1,
                        mask: Arc::new(vec![0]),
                    }
                }
                4 => Arc::make_mut(&mut doc.layers[1].pixels)
                    .as_chunks_mut::<4>()
                    .0
                    .fill([options.color[0], options.color[1], options.color[2], 255]),
                _ => options.operation = BrushOperation::Erase,
            }
            let original = doc.clone();
            let mut stroke = Stroke::begin(&mut doc, options, point(1.));
            assert!(stroke.take_damage().is_none());
            assert_eq!(doc.revision, original.revision);
            assert!(Arc::ptr_eq(
                &doc.layers[1].pixels,
                &original.layers[1].pixels
            ));
            assert_eq!(stroke.scratch_capacity_bytes(), 0);
        }
        let mut doc = Document::new(128, 128);
        let mut stroke = Stroke::begin(&mut doc, brush(), point(1.));
        assert_eq!(
            stroke.scratch_capacity_bytes(),
            CELLS * 4 + 4 * std::mem::size_of::<Option<Box<[f32; CELLS]>>>()
        );
        let capacity = stroke.scratch_capacity_bytes();
        hold(&mut doc, &mut stroke);
        assert_eq!(stroke.scratch_capacity_bytes(), capacity);
        stroke.extend(
            &mut doc,
            Point {
                x: 80.5,
                ..point(1.)
            },
        );
        assert_eq!(stroke.scratch_capacity_bytes(), capacity + CELLS * 4);
        let snapshot = doc.clone();
        doc.active = 0;
        stroke.extend(
            &mut doc,
            Point {
                x: 100.5,
                ..point(1.)
            },
        );
        assert_eq!(doc.layers, snapshot.layers);
    }
    #[test]
    fn tiled_coverage_clips_partial_edge_tiles_and_rejects_changed_dimensions() {
        for (width, height) in [(1, 1), (63, 65), (65, 67), (129, 193)] {
            let mut doc = Document::new(width, height);
            let options = Brush {
                size: 512.,
                tip: BrushTip::Flat,
                flow: 1.,
                ..brush()
            };
            let center = Point {
                x: width as f32 * 0.5,
                y: height as f32 * 0.5,
                pressure: 1.,
            };
            let mut stroke = Stroke::begin(&mut doc, options, center);
            let slots = (width as usize).div_ceil(SIDE) * (height as usize).div_ceil(SIDE);
            let capacity = slots
                * (std::mem::size_of::<Option<Box<[f32; CELLS]>>>()
                    + std::mem::size_of::<[f32; CELLS]>());
            assert_eq!(stroke.scratch_capacity_bytes(), capacity);
            assert!(
                doc.layers[1]
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|pixel| pixel[3] == 102)
            );
            let original = doc.clone();
            hold(&mut doc, &mut stroke);
            assert_eq!(doc, original);
            assert_eq!(stroke.scratch_capacity_bytes(), capacity);
            let mut other = Document::new(width + 1, height);
            let untouched = other.clone();
            stroke.take_damage();
            stroke.spray(&mut other, 0.1, 1.);
            assert!(stroke.take_damage().is_none());
            assert_eq!(other, untouched);
        }
    }
    #[test]
    fn partitioned_input_and_unsupported_operations_preserve_pixels() {
        let render = |pieces| {
            let mut doc = Document::new(128, 64);
            let options = Brush {
                spacing: 0.125,
                pressure_opacity: true,
                ..brush()
            };
            let at = |step: usize| Point {
                x: 16. + 96. * step as f32 / pieces as f32,
                y: 32.,
                pressure: 0.25 + 0.75 * step as f32 / pieces as f32,
            };
            let mut stroke = Stroke::begin(&mut doc, options, at(0));
            for step in 1..=pieces {
                stroke.extend(&mut doc, at(step));
            }
            doc
        };
        let single = render(1);
        for pieces in [2, 8, 32, 128] {
            assert_eq!(single.layers, render(pieces).layers);
        }
        for operation in [
            BrushOperation::Blend,
            BrushOperation::Blur,
            BrushOperation::Smudge,
        ] {
            let render = |opacity_mode| {
                let mut doc = Document::new(128, 64);
                for (index, pixel) in Arc::make_mut(&mut doc.layers[1].pixels)
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .enumerate()
                {
                    *pixel = [(index % 251) as u8, 90, 210, 180];
                }
                let options = Brush {
                    operation,
                    opacity_mode,
                    ..brush()
                };
                let mut stroke = Stroke::begin(&mut doc, options, point(1.));
                stroke.extend(
                    &mut doc,
                    Point {
                        x: 64.,
                        ..point(1.)
                    },
                );
                (doc, stroke.scratch_capacity_bytes())
            };
            let (legacy, original_scratch) = render(OpacityMode::PerDab);
            let (retained, retained_scratch) = render(OpacityMode::PerStroke);
            assert_eq!(legacy.layers, retained.layers);
            assert_eq!(original_scratch, retained_scratch);
        }
    }
}
