use parfait_core::{
    Brush, BrushOperation, BrushTip, Document, History, LayerCombine, LayerId,
    OpacityMode, PixelRect, Point, Stroke,
};
use std::sync::Arc;

fn point(column: f32, row: f32) -> Point {
    Point { x: column, y: row, pressure: 1. }
}

#[test]
fn paint_and_white_erase_preserve_partial_alpha_with_exact_color_interpolation()
{
    for operation in [BrushOperation::Paint, BrushOperation::Erase] {
        for opacity_mode in [OpacityMode::PerDab, OpacityMode::PerStroke] {
            let mut document = Document::new(16, 16);
            document.layers[1].alpha_locked = true;
            let pixels = Arc::make_mut(&mut document.layers[1].pixels);
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel.copy_from_slice(&[7, 8, 9, 0]);
            }
            let index = (8 * 16 + 8) * 4;
            pixels[index..index + 4].copy_from_slice(&[40, 80, 120, 64]);
            let before = document.clone();
            Stroke::begin(
                &mut document,
                Brush {
                    operation,
                    opacity_mode,
                    size: 8.,
                    opacity: 0.5,
                    flow: 1.,
                    color: [200, 40, 80],
                    pressure: false,
                    ..Default::default()
                },
                point(8.5, 8.5),
            );
            let expected = if operation == BrushOperation::Paint {
                [120, 60, 100, 64]
            } else {
                [148, 168, 188, 64]
            };
            assert_eq!(&document.layers[1].pixels[index..index + 4], &expected);
            assert_eq!(&document.layers[1].pixels[..4], &[7, 8, 9, 0]);
            assert!(
                document.layers[1]
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(before.layers[1].pixels.as_chunks::<4>().0)
                    .all(|(after, before)| after[3] == before[3]
                        && (before[3] != 0 || after == before))
            );
        }
    }
}

#[test]
fn every_operation_preserves_alpha_and_incremental_composition() {
    for operation in [
        BrushOperation::Paint,
        BrushOperation::Erase,
        BrushOperation::Blend,
        BrushOperation::Blur,
        BrushOperation::Smudge,
    ] {
        for opacity_mode in [OpacityMode::PerDab, OpacityMode::PerStroke] {
            let mut document = Document::new(40, 24);
            document.layers[1].alpha_locked = true;
            for (index, pixel) in Arc::make_mut(&mut document.layers[1].pixels)
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .enumerate()
            {
                let column = index % 40;
                pixel.copy_from_slice(&[
                    (index * 31) as u8,
                    (index * 17) as u8,
                    (index * 9) as u8,
                    if column < 24 { [0, 32, 128, 255][column % 4] } else { 0 },
                ]);
            }
            let before = document.clone();
            let mut cached = document.composite();
            let brush = Brush {
                operation,
                opacity_mode,
                size: 12.,
                opacity: 0.7,
                flow: 0.3,
                tip: BrushTip::Soft,
                angle: 23.,
                aspect: 0.7,
                color: [180, 30, 70],
                ..Default::default()
            };
            let mut stroke =
                Stroke::begin(&mut document, brush, point(4., 12.));
            for column in 4..34 {
                stroke.extend(
                    &mut document,
                    Point {
                        x: column as f32,
                        y: 12.,
                        pressure: [0.2, 0.5, 1.][column % 3],
                    },
                );
                if let Some(rect) = stroke.take_damage() {
                    let patch = document.composite_region(rect);
                    let stride = (rect.x1 - rect.x0) as usize * 4;
                    for (row, pixels) in patch.chunks_exact(stride).enumerate()
                    {
                        let start = ((rect.y0 as usize + row) * 40
                            + rect.x0 as usize)
                            * 4;
                        cached[start..start + stride].copy_from_slice(pixels);
                    }
                }
                assert_eq!(cached, document.composite());
            }
            assert!(
                document.layers[1]
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(before.layers[1].pixels.as_chunks::<4>().0)
                    .all(|(after, before)| after[3] == before[3]
                        && (before[3] != 0 || after == before))
            );
            assert_ne!(
                document.layers[1].pixels, before.layers[1].pixels,
                "{operation:?}/{opacity_mode:?}"
            );
            assert_eq!(
                document.composite_region(PixelRect {
                    x0: 0,
                    y0: 0,
                    x1: 40,
                    y1: 24
                }),
                document.composite()
            );
        }
    }
}

#[test]
fn ineffective_color_and_carried_samples_do_not_copy_pixels_or_dirty_history() {
    for operation in [
        BrushOperation::Paint,
        BrushOperation::Erase,
        BrushOperation::Blend,
        BrushOperation::Blur,
        BrushOperation::Smudge,
    ] {
        for opacity_mode in [OpacityMode::PerDab, OpacityMode::PerStroke] {
            let mut document = Document::new(64, 32);
            document.layers[1].alpha_locked = true;
            for (index, pixel) in Arc::make_mut(&mut document.layers[1].pixels)
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .enumerate()
            {
                pixel.copy_from_slice(if index % 64 < 24 {
                    &[255; 4]
                } else {
                    &[9, 17, 23, 0]
                });
            }
            let before = document.clone();
            let mut history = History::default();
            history.begin(&document, "Ineffective stroke");
            let mut stroke = Stroke::begin(
                &mut document,
                Brush {
                    operation,
                    opacity_mode,
                    color: [255; 3],
                    size: 10.,
                    pressure: false,
                    color_stretch: 1.,
                    ..Default::default()
                },
                point(10., 16.),
            );
            for column in 10..54 {
                stroke.extend(&mut document, point(column as f32, 16.));
            }
            assert_eq!(document, before, "{operation:?}/{opacity_mode:?}");
            assert!(Arc::ptr_eq(
                &document.layers[1].pixels,
                &before.layers[1].pixels
            ));
            assert!(stroke.take_damage().is_none());
            assert!(!history.commit(&document));
            if matches!(
                operation,
                BrushOperation::Paint | BrushOperation::Erase
            ) {
                assert_eq!(stroke.scratch_capacity_bytes(), 0);
            }
        }
    }
}

#[test]
fn capped_recolor_stays_at_the_baseline_opacity_limit() {
    let mut document = Document::new(24, 24);
    document.layers[1].alpha_locked = true;
    for pixel in
        Arc::make_mut(&mut document.layers[1].pixels).as_chunks_mut::<4>().0
    {
        pixel.copy_from_slice(&[0, 0, 0, 64]);
    }
    let mut stroke = Stroke::begin(
        &mut document,
        Brush {
            size: 12.,
            color: [255; 3],
            opacity: 0.25,
            flow: 0.2,
            opacity_mode: OpacityMode::PerStroke,
            pressure: false,
            continuous_spray: true,
            ..Default::default()
        },
        point(12., 12.),
    );
    for _ in 0..20 {
        stroke.spray(&mut document, 0.1, 1.);
    }
    assert!(
        document.layers[1]
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 64
                && pixel[..3].iter().all(|channel| *channel <= 64))
    );
    assert!(
        document.layers[1]
            .pixels
            .iter()
            .any(|channel| *channel > 0 && *channel < 64)
    );
}

#[test]
fn ids_stay_unique_and_history_never_reuses_a_discarded_branch_identifier() {
    let mut document = Document::new(4, 4);
    let mut history = History::default();
    history.begin(&document, "A");
    document.add_layer();
    history.commit(&document);
    let discarded = document.layers[2].id;
    assert!(history.undo(&mut document));
    history.begin(&document, "B");
    document.add_layer();
    history.commit(&document);
    assert!(document.layers[2].id.0 > discarded.0);
    assert!(!history.can_redo());
    let id = document.layers[2].id;
    history.begin(&document, "Cancelled");
    document.add_layer();
    let cancelled = document.layers[3].id;
    history.cancel(&mut document);
    document.add_layer();
    assert!(document.layers[3].id.0 > cancelled.0);
    document.layers.swap(1, 2);
    assert_eq!(document.layer_index(id), Some(1));
    assert!(document.validate());
    document.layers[0].id = document.layers[1].id;
    assert!(!document.validate());
    document.layers[0].id = LayerId(0);
    assert!(!document.validate());
}

#[test]
fn frozen_strokes_and_destructive_commands_respect_identity_and_alpha_policy() {
    let mut different_target = Document::new(16, 16);
    let mut frozen = Stroke::begin(
        &mut different_target,
        Brush { size: 4., pressure: false, ..Default::default() },
        point(2., 2.),
    );
    different_target.add_layer();
    let target_before = different_target.clone();
    frozen.extend(&mut different_target, point(12., 12.));
    assert_eq!(different_target, target_before);
    let mut document = Document::new(8, 8);
    let mut stroke = Stroke::begin(
        &mut document,
        Brush { opacity: 0., ..Default::default() },
        point(1., 1.),
    );
    document.layers[1].alpha_locked = true;
    let before = document.clone();
    stroke.extend(&mut document, point(6., 6.));
    assert_eq!(document, before);
    for action in [LayerCombine::Clear, LayerCombine::TransferDown] {
        assert!(document.layer_combine_reason(action, 2).is_some());
        assert!(!document.combine_layers(action, 2));
        assert_eq!(document, before);
    }
    document.next_layer_id = u64::MAX;
    let before = document.clone();
    document.add_layer();
    assert_eq!(document, before);
    assert!(
        document.layer_combine_reason(LayerCombine::CopyVisible, 2).is_some()
    );
}
