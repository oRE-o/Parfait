use crate::{
    Brush, BrushOperation, Document, PixelRect, Point,
    brush::{Dab, Destination, recolor_locked},
};

#[inline]
fn selection_strength<const SELECTED: bool>(
    selection: Option<&crate::MaskCoverage>,
    index: usize,
    amount: f32,
) -> f32 {
    if SELECTED {
        crate::selection::strength(selection, index, amount)
    } else {
        amount
    }
}
#[cfg(test)]
use std::sync::Arc;

#[derive(Default)]
pub(crate) enum Scratch {
    #[default]
    None,
    Wash(crate::wash::Wash),
    Blur(BlurScratch),
    Smudge(crate::smudge::Patch),
    Mask(crate::layer_mask::paint::MaskPaint),
}
pub(crate) struct BlurScratch {
    source: Vec<[u8; 4]>,
    horizontal: Vec<[u16; 4]>,
}
impl Scratch {
    pub fn new(brush: &Brush, doc: &Document) -> Self {
        let side = brush.size.clamp(1., 512.).ceil() as usize + 4;
        match brush.operation() {
            BrushOperation::Paint | BrushOperation::Erase
                if brush.opacity_mode == crate::OpacityMode::PerStroke =>
            {
                Self::Wash(crate::wash::Wash::new(doc))
            }
            BrushOperation::Blur => {
                let halo = brush.blur_radius_pixels(brush.size / 2.) * 2;
                Self::Blur(BlurScratch {
                    source: Vec::with_capacity((side + halo).pow(2)),
                    horizontal: Vec::with_capacity(side * (side + halo)),
                })
            }
            BrushOperation::Smudge => Self::Smudge(crate::smudge::Patch::new(side)),
            _ => Self::None,
        }
    }
    pub fn capacity_bytes(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Wash(wash) => wash.capacity_bytes(),
            Self::Blur(scratch) => {
                scratch.source.capacity() * 4 + scratch.horizontal.capacity() * 8
            }
            Self::Smudge(patch) => patch.capacity_bytes(),
            Self::Mask(mask) => mask.capacity_bytes(),
        }
    }
}

fn premultiplied(pixel: &[u8]) -> [f32; 4] {
    let alpha = f32::from(pixel[3]) / 255.;
    [
        f32::from(pixel[0]) * alpha,
        f32::from(pixel[1]) * alpha,
        f32::from(pixel[2]) * alpha,
        alpha,
    ]
}
pub(crate) fn interpolate(first: [f32; 4], second: [f32; 4], amount: f32) -> [f32; 4] {
    std::array::from_fn(|channel| first[channel] * (1. - amount) + second[channel] * amount)
}
pub(crate) fn apply(pixel: &mut [u8], sample: [f32; 4], amount: f32) -> bool {
    if amount <= 0. {
        return false;
    }
    let mixed = interpolate(premultiplied(pixel), sample, amount);
    let alpha = (mixed[3] * 255.).round() as u8;
    let result = if alpha == 0 {
        [0; 4]
    } else {
        [
            (mixed[0] / mixed[3]).round() as u8,
            (mixed[1] / mixed[3]).round() as u8,
            (mixed[2] / mixed[3]).round() as u8,
            alpha,
        ]
    };
    if pixel == result {
        return false;
    }
    pixel.copy_from_slice(&result);
    true
}

#[inline]
pub(crate) fn apply_locked(
    pixel: &mut [u8],
    sample: [f32; 4],
    amount: f32,
    alpha_locked: bool,
) -> bool {
    if !alpha_locked {
        return apply(pixel, sample, amount);
    }
    if sample[3] <= 0. {
        return false;
    }
    let color = [sample[0], sample[1], sample[2]].map(|value| (value / sample[3]).round() as u8);
    recolor_locked(pixel, color, amount * sample[3])
}

pub(crate) fn blend(
    doc: &mut Document,
    brush: &Brush,
    point: Point,
    carry: &mut Option<[f32; 4]>,
) -> Option<PixelRect> {
    if doc.selection.is_none() {
        blend_selected::<false>(doc, brush, point, carry)
    } else {
        blend_selected::<true>(doc, brush, point, carry)
    }
}

fn blend_selected<const SELECTED: bool>(
    doc: &mut Document,
    brush: &Brush,
    point: Point,
    carry: &mut Option<[f32; 4]>,
) -> Option<PixelRect> {
    let dab = Dab::new(doc, brush, point)?;
    if brush.opacity == 0. {
        return None;
    }
    let source = &doc.layers[doc.active].pixels;
    let mut total = [0.; 4];
    let mut weight = 0.;
    for row in dab.bounds.y0..dab.bounds.y1 {
        for column in dab.bounds.x0..dab.bounds.x1 {
            let coverage = dab.coverage(brush, column, row);
            let index = ((row * doc.width + column) * 4) as usize;
            let sample = premultiplied(&source[index..index + 4]);
            for channel in 0..4 {
                total[channel] += sample[channel] * coverage;
            }
            weight += coverage;
        }
    }
    if weight <= 0. {
        return None;
    }
    let sample = total.map(|component| component / weight);
    let previous = carry.replace(sample)?;
    let sample = interpolate(sample, previous, brush.color_stretch);
    *carry = Some(sample);
    if sample[3] == 0. && total[3] == 0. {
        return None;
    }
    let alpha_locked = doc.layers[doc.active].alpha_locked;
    let mut destination = Destination::Shared(&mut doc.layers[doc.active].pixels);
    let mut changed = false;
    for row in dab.bounds.y0..dab.bounds.y1 {
        for column in dab.bounds.x0..dab.bounds.x1 {
            let index = ((row * doc.width + column) * 4) as usize;
            let (next, pixel_changed) = destination.edit(index, |pixel| {
                apply_locked(
                    pixel,
                    sample,
                    selection_strength::<SELECTED>(
                        doc.selection.as_ref(),
                        index / 4,
                        dab.alpha(brush, column, row),
                    ),
                    alpha_locked,
                )
            });
            destination = next;
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

pub(crate) fn integer_premultiplied(pixel: [u8; 4]) -> [u32; 4] {
    let alpha = u32::from(pixel[3]);
    [
        u32::from(pixel[0]) * alpha,
        u32::from(pixel[1]) * alpha,
        u32::from(pixel[2]) * alpha,
        alpha * 255,
    ]
}

pub(crate) fn blur(
    doc: &mut Document,
    brush: &Brush,
    point: Point,
    scratch: &mut Scratch,
) -> Option<PixelRect> {
    if doc.selection.is_none() {
        blur_selected::<false>(doc, brush, point, scratch)
    } else {
        blur_selected::<true>(doc, brush, point, scratch)
    }
}

fn blur_selected<const SELECTED: bool>(
    doc: &mut Document,
    brush: &Brush,
    point: Point,
    scratch: &mut Scratch,
) -> Option<PixelRect> {
    let Scratch::Blur(scratch) = scratch else {
        return None;
    };
    let dab = Dab::new(doc, brush, point)?;
    if brush.opacity == 0. {
        return None;
    }
    let radius = brush.blur_radius_pixels(dab.radius) as u32;
    let source_bounds = PixelRect {
        x0: dab.bounds.x0.saturating_sub(radius),
        y0: dab.bounds.y0.saturating_sub(radius),
        x1: (dab.bounds.x1 + radius).min(doc.width),
        y1: (dab.bounds.y1 + radius).min(doc.height),
    };
    let source_width = (source_bounds.x1 - source_bounds.x0) as usize;
    let source_height = (source_bounds.y1 - source_bounds.y0) as usize;
    let target_width = (dab.bounds.x1 - dab.bounds.x0) as usize;
    scratch.source.clear();
    let source = &doc.layers[doc.active].pixels;
    for row in source_bounds.y0..source_bounds.y1 {
        let start = ((row * doc.width + source_bounds.x0) * 4) as usize;
        scratch.source.extend(
            source[start..start + source_width * 4]
                .as_chunks::<4>()
                .0
                .iter()
                .copied(),
        );
    }
    if scratch.source.iter().all(|pixel| pixel[3] == 0)
        || scratch
            .source
            .iter()
            .all(|pixel| *pixel == scratch.source[0])
    {
        return None;
    }
    scratch
        .horizontal
        .resize(target_width * source_height, [0; 4]);
    for row in 0..source_height {
        let read = |column: u32| {
            integer_premultiplied(
                scratch.source[row * source_width + (column - source_bounds.x0) as usize],
            )
        };
        let mut left = dab.bounds.x0.saturating_sub(radius);
        let mut right = (dab.bounds.x0 + radius + 1).min(doc.width);
        let mut sum = [0u32; 4];
        for column in left..right {
            let sample = read(column);
            for channel in 0..4 {
                sum[channel] += sample[channel];
            }
        }
        for column in dab.bounds.x0..dab.bounds.x1 {
            let count = right - left;
            scratch.horizontal[row * target_width + (column - dab.bounds.x0) as usize] =
                sum.map(|component| ((component + count / 2) / count) as u16);
            let next_left = (column + 1).saturating_sub(radius);
            let next_right = (column + radius + 2).min(doc.width);
            if next_left > left {
                let sample = read(left);
                for channel in 0..4 {
                    sum[channel] -= sample[channel];
                }
            }
            if next_right > right && column + 1 < dab.bounds.x1 {
                let sample = read(right);
                for channel in 0..4 {
                    sum[channel] += sample[channel];
                }
            }
            left = next_left;
            right = next_right;
        }
    }
    let alpha_locked = doc.layers[doc.active].alpha_locked;
    let mut destination = Destination::Shared(&mut doc.layers[doc.active].pixels);
    let mut changed = false;
    for column in dab.bounds.x0..dab.bounds.x1 {
        let read = |row: u32| {
            scratch.horizontal[(row - source_bounds.y0) as usize * target_width
                + (column - dab.bounds.x0) as usize]
                .map(u32::from)
        };
        let mut top = dab.bounds.y0.saturating_sub(radius);
        let mut bottom = (dab.bounds.y0 + radius + 1).min(doc.height);
        let mut sum = [0u32; 4];
        for row in top..bottom {
            let sample = read(row);
            for channel in 0..4 {
                sum[channel] += sample[channel];
            }
        }
        for row in dab.bounds.y0..dab.bounds.y1 {
            let count = (bottom - top) as f32;
            let sample = [
                sum[0] as f32 / count / 255.,
                sum[1] as f32 / count / 255.,
                sum[2] as f32 / count / 255.,
                sum[3] as f32 / count / 65025.,
            ];
            let index = ((row * doc.width + column) * 4) as usize;
            let (next, pixel_changed) = destination.edit(index, |pixel| {
                apply_locked(
                    pixel,
                    sample,
                    selection_strength::<SELECTED>(
                        doc.selection.as_ref(),
                        index / 4,
                        dab.alpha(brush, column, row),
                    ),
                    alpha_locked,
                )
            });
            destination = next;
            changed |= pixel_changed;
            let next_top = (row + 1).saturating_sub(radius);
            let next_bottom = (row + radius + 2).min(doc.height);
            if next_top > top {
                let sample = read(top);
                for channel in 0..4 {
                    sum[channel] -= sample[channel];
                }
            }
            if next_bottom > bottom && row + 1 < dab.bounds.y1 {
                let sample = read(bottom);
                for channel in 0..4 {
                    sum[channel] += sample[channel];
                }
            }
            top = next_top;
            bottom = next_bottom;
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
    use crate::{BrushTip, History, Stroke};

    fn fixture() -> Document {
        let mut document = Document::new(17, 13);
        for (index, pixel) in Arc::make_mut(&mut document.layers[1].pixels)
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            *pixel = [
                (index * 73) as u8,
                (index * 31) as u8,
                (index * 107) as u8,
                (index * 43) as u8,
            ];
        }
        document
    }

    fn reference_blur(document: &mut Document, brush: &Brush, point: Point) {
        let dab = Dab::new(document, brush, point).unwrap();
        let radius = brush.blur_radius_pixels(dab.radius) as u32;
        let source = document.layers[document.active].pixels.clone();
        let mut changed = false;
        let pixels = Arc::make_mut(&mut document.layers[document.active].pixels);
        for row in dab.bounds.y0..dab.bounds.y1 {
            for column in dab.bounds.x0..dab.bounds.x1 {
                let left = column.saturating_sub(radius);
                let right = (column + radius + 1).min(document.width);
                let top = row.saturating_sub(radius);
                let bottom = (row + radius + 1).min(document.height);
                let mut sum = [0u32; 4];
                for source_row in top..bottom {
                    let mut horizontal = [0u32; 4];
                    for source_column in left..right {
                        let index = ((source_row * document.width + source_column) * 4) as usize;
                        let sample =
                            integer_premultiplied(source[index..index + 4].try_into().unwrap());
                        for channel in 0..4 {
                            horizontal[channel] += sample[channel];
                        }
                    }
                    let count = right - left;
                    for channel in 0..4 {
                        sum[channel] += (horizontal[channel] + count / 2) / count;
                    }
                }
                let count = (bottom - top) as f32;
                let sample = [
                    sum[0] as f32 / count / 255.,
                    sum[1] as f32 / count / 255.,
                    sum[2] as f32 / count / 255.,
                    sum[3] as f32 / count / 65025.,
                ];
                let index = ((row * document.width + column) * 4) as usize;
                changed |= apply(
                    &mut pixels[index..index + 4],
                    sample,
                    crate::selection::strength(
                        document.selection.as_ref(),
                        index / 4,
                        dab.alpha(brush, column, row),
                    ),
                );
            }
        }
        if changed {
            document.touch();
        }
    }

    #[test]
    fn local_blur_matches_slow_reference_at_edges_for_every_tip_and_radius() {
        for tip in [
            BrushTip::Round,
            BrushTip::Soft,
            BrushTip::Flat,
            BrushTip::Pencil,
            BrushTip::Image {
                width: 2,
                height: 2,
                mask: Arc::new(vec![0, 80, 170, 255]),
            },
        ] {
            for radius in [1., 2., 7., 32.] {
                for center in [[8.2, 6.7], [0., 0.], [16.8, 12.9], [-3., 5.]] {
                    let brush = Brush {
                        operation: BrushOperation::Blur,
                        tip: tip.clone(),
                        size: 15.,
                        aspect: 0.6,
                        angle: 38.,
                        opacity: 0.7,
                        pressure_opacity: true,
                        blur_automatic: false,
                        blur_radius: radius,
                        ..Default::default()
                    };
                    let point = Point {
                        x: center[0],
                        y: center[1],
                        pressure: 0.6,
                    };
                    let mut actual = fixture();
                    let mut expected = actual.clone();
                    Stroke::begin(&mut actual, brush.clone(), point);
                    reference_blur(&mut expected, &brush, point);
                    assert_eq!(actual, expected, "radius={radius}, center={center:?}");
                }
            }
        }
    }

    #[test]
    fn color_blend_carries_start_color_without_adding_drawing_pigment() {
        let mut original = Document::new(40, 16);
        for row in 0..16 {
            for column in 0..40 {
                let index = ((row * 40 + column) * 4) as usize;
                Arc::make_mut(&mut original.layers[1].pixels)[index..index + 4].copy_from_slice(
                    if column < 12 {
                        &[255, 0, 0, 255]
                    } else {
                        &[0, 0, 255, 255]
                    },
                );
            }
        }
        let paint = |stretch, color| {
            let mut document = original.clone();
            let mut stroke = Stroke::begin(
                &mut document,
                Brush {
                    operation: BrushOperation::Blend,
                    color_stretch: stretch,
                    color,
                    size: 6.,
                    spacing: 0.5,
                    pressure: false,
                    ..Default::default()
                },
                Point {
                    x: 4.,
                    y: 8.,
                    pressure: 1.,
                },
            );
            assert_eq!(document, original);
            assert!(stroke.take_damage().is_none());
            stroke.extend(
                &mut document,
                Point {
                    x: 34.,
                    y: 8.,
                    pressure: 1.,
                },
            );
            document
        };
        let retained = paint(1., [0, 255, 0]);
        let local = paint(0., [0, 255, 0]);
        let index = (8 * 40 + 31) * 4;
        assert!(retained.layers[1].pixels[index] > local.layers[1].pixels[index] + 40);
        assert_eq!(retained, paint(1., [255, 255, 0]));
        assert!(
            retained.layers[1]
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[1] == 0)
        );
    }

    #[test]
    fn transparent_rgb_does_not_pollute_blur_and_radius_changes_real_pixels() {
        let mut original = Document::new(32, 16);
        for (index, pixel) in Arc::make_mut(&mut original.layers[1].pixels)
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            *pixel = if index % 32 < 16 {
                [0, 0, 255, 255]
            } else {
                [255, 0, 0, 0]
            };
        }
        let blur = |radius| {
            let mut document = original.clone();
            Stroke::begin(
                &mut document,
                Brush {
                    operation: BrushOperation::Blur,
                    pressure: false,
                    size: 30.,
                    blur_automatic: false,
                    blur_radius: radius,
                    ..Default::default()
                },
                Point {
                    x: 16.,
                    y: 8.,
                    pressure: 1.,
                },
            );
            document
        };
        let narrow = blur(1.);
        let wide = blur(6.);
        let index = (8 * 32 + 19) * 4;
        assert_eq!(narrow.layers[1].pixels[index + 3], 0);
        assert!(wide.layers[1].pixels[index + 3] > 0);
        assert_eq!(&wide.layers[1].pixels[index..index + 3], &[0, 0, 255]);
    }

    #[test]
    fn blending_respects_layer_state_and_empty_strokes_do_not_change_revisions_or_buffers() {
        for operation in [BrushOperation::Blend, BrushOperation::Blur] {
            for blocked in 0..4 {
                let mut document = if blocked == 0 {
                    Document::new(17, 13)
                } else {
                    fixture()
                };
                document.layers[1].locked = blocked == 1;
                document.layers[1].visible = blocked != 2;
                let original = document.clone();
                let mut stroke = Stroke::begin(
                    &mut document,
                    Brush {
                        operation,
                        opacity: if blocked == 3 { 0. } else { 1. },
                        ..Default::default()
                    },
                    Point {
                        x: 5.,
                        y: 5.,
                        pressure: 1.,
                    },
                );
                stroke.extend(
                    &mut document,
                    Point {
                        x: 12.,
                        y: 8.,
                        pressure: 1.,
                    },
                );
                assert_eq!(document, original);
                assert!(Arc::ptr_eq(
                    &document.layers[1].pixels,
                    &original.layers[1].pixels
                ));
                assert!(stroke.take_damage().is_none());
            }
        }
    }

    #[test]
    fn blending_strength_pressure_and_automatic_radius_change_pixels() {
        for operation in [
            BrushOperation::Blend,
            BrushOperation::Blur,
            BrushOperation::Smudge,
        ] {
            let render = |flow, pressure| {
                let mut document = fixture();
                let mut stroke = Stroke::begin(
                    &mut document,
                    Brush {
                        operation,
                        size: 14.,
                        flow,
                        pressure_opacity: true,
                        pressure: true,
                        ..Default::default()
                    },
                    Point {
                        x: 3.,
                        y: 6.,
                        pressure,
                    },
                );
                stroke.extend(
                    &mut document,
                    Point {
                        x: 13.,
                        y: 6.,
                        pressure,
                    },
                );
                document
            };
            assert!(render(0.1, 1.).layers != render(1., 1.).layers);
            assert!(render(1., 0.2).layers != render(1., 1.).layers);
        }
        let brush = Brush {
            operation: BrushOperation::Blur,
            size: 200.,
            ..Default::default()
        };
        assert_eq!(brush.blur_radius_pixels(100.), 10);
        assert_eq!(brush.blur_radius_pixels(5.), 1);
        assert_eq!(
            Brush {
                blur_automatic: false,
                blur_radius: 17.,
                ..brush
            }
            .blur_radius_pixels(5.),
            17
        );
        assert_eq!(
            Brush {
                erase: true,
                operation: BrushOperation::Blend,
                ..Default::default()
            }
            .operation(),
            BrushOperation::Erase
        );
    }

    #[test]
    fn scratch_is_bounded_reused_and_absent_from_paint_and_blend() {
        for operation in [
            BrushOperation::Paint,
            BrushOperation::Erase,
            BrushOperation::Blend,
            BrushOperation::Blur,
        ] {
            let mut document = Document::new(640, 640);
            Arc::make_mut(&mut document.layers[1].pixels).fill(128);
            let mut stroke = Stroke::begin(
                &mut document,
                Brush {
                    operation,
                    size: 512.,
                    pressure: false,
                    blur_automatic: false,
                    blur_radius: 32.,
                    ..Default::default()
                },
                Point {
                    x: 320.,
                    y: 320.,
                    pressure: 1.,
                },
            );
            let capacity = stroke.scratch_capacity_bytes();
            if operation == BrushOperation::Blur {
                assert!((1..=3_800_000).contains(&capacity));
            } else {
                assert_eq!(capacity, 0);
            }
            stroke.extend(
                &mut document,
                Point {
                    x: 500.,
                    y: 400.,
                    pressure: 1.,
                },
            );
            assert_eq!(stroke.scratch_capacity_bytes(), capacity);
        }
        for invalid in [f32::NAN, f32::INFINITY, -1., 2.] {
            assert!(
                !Brush {
                    color_stretch: invalid,
                    ..Default::default()
                }
                .validate()
            );
        }
        assert!(
            !Brush {
                blur_radius: 33.,
                ..Default::default()
            }
            .validate()
        );
    }

    #[test]
    fn blended_artwork_undo_redo_and_event_segmentation_remain_equivalent() {
        for operation in [BrushOperation::Blend, BrushOperation::Blur] {
            let original = fixture();
            let mut whole = original.clone();
            let mut segmented = original.clone();
            let brush = Brush {
                operation,
                size: 8.,
                pressure: false,
                spacing: 0.25,
                continuous_spray: true,
                ..Default::default()
            };
            let begin = Point {
                x: 2.,
                y: 6.,
                pressure: 1.,
            };
            let mut history = History::default();
            history.checkpoint(&whole);
            let mut first = Stroke::begin(&mut whole, brush.clone(), begin);
            let mut second = Stroke::begin(&mut segmented, brush, begin);
            first.extend(&mut whole, Point { x: 14., ..begin });
            for column in 3..=14 {
                second.extend(
                    &mut segmented,
                    Point {
                        x: column as f32,
                        ..begin
                    },
                );
            }
            first.spray(&mut whole, 0.1, 1.);
            second.spray(&mut segmented, 0.1, 1.);
            assert_eq!(whole, segmented);
            let result = whole.clone();
            history.undo(&mut whole);
            assert_eq!(whole.layers, original.layers);
            history.redo(&mut whole);
            assert_eq!(whole.layers, result.layers);
        }
    }
}
