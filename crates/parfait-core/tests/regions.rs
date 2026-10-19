use parfait_core::{Brush, BrushOperation, BrushTip, Document, PixelRect, Point, Stroke};
use std::sync::Arc;

#[test]
fn incremental_pixels_match_full_composite_for_all_tips_and_operations() {
    check_incremental_pixels(parfait_core::OpacityMode::PerDab);
    check_incremental_pixels(parfait_core::OpacityMode::PerStroke);
}

fn check_incremental_pixels(opacity_mode: parfait_core::OpacityMode) {
    for tip in [
        BrushTip::Round,
        BrushTip::Soft,
        BrushTip::Pencil,
        BrushTip::Flat,
        BrushTip::Image {
            width: 2,
            height: 2,
            mask: Arc::new(vec![0, 80, 170, 255]),
        },
    ] {
        for operation in [
            BrushOperation::Paint,
            BrushOperation::Erase,
            BrushOperation::Blend,
            BrushOperation::Blur,
            BrushOperation::Smudge,
        ] {
            let mut doc = Document::new(96, 80);
            doc.layers[0].visible = false;
            doc.layers[1].opacity = 0.63;
            for (index, pixel) in Arc::make_mut(&mut doc.layers[1].pixels)
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .enumerate()
            {
                pixel.copy_from_slice(&[(index % 251) as u8, 120, (index % 173) as u8, 180]);
            }
            let mut cached = doc.composite();
            let brush = Brush {
                tip: tip.clone(),
                operation,
                opacity_mode,
                size: 27.,
                angle: 37.,
                aspect: 0.5,
                pressure_opacity: true,
                gap_mode: parfait_core::GapMode::Fixed,
                pressure_spacing: true,
                minimum_spacing: 0.2,
                spacing_curve: [1., 0.8, 0.4, 0.2, 0.].into(),
                continuous_spray: true,
                pressure_curve: parfait_core::ResponseCurve {
                    points: vec![[0., 0.], [0.17, 0.9], [0.62, 0.2], [1., 1.]],
                    controls: Some(vec![
                        [[0.04, 0.8], [0.13, 0.9]],
                        [[0.3, 0.8], [0.5, 0.3]],
                        [[0.8, 0.1], [0.95, 0.9]],
                    ]),
                },
                ..Default::default()
            };
            let mut stroke = Stroke::begin(
                &mut doc,
                brush,
                Point {
                    x: -10.,
                    y: 3.,
                    pressure: 0.5,
                },
            );
            for i in 0..30 {
                stroke.extend(
                    &mut doc,
                    Point {
                        x: i as f32 * 4.,
                        y: i as f32 * 3.,
                        pressure: (i % 10) as f32 / 10.,
                    },
                );
                stroke.spray(&mut doc, 0.05, (i % 10) as f32 / 10.);
                if let Some(r) = stroke.take_damage() {
                    let patch = doc.composite_region(r);
                    let stride = (r.x1 - r.x0) as usize * 4;
                    for (y, row) in patch.chunks_exact(stride).enumerate() {
                        let at = ((r.y0 as usize + y) * doc.width as usize + r.x0 as usize) * 4;
                        cached[at..at + stride].copy_from_slice(row);
                    }
                }
                assert_eq!(cached, doc.composite());
                assert!(stroke.take_damage().is_none());
            }
        }
    }
}

#[test]
fn empty_and_outside_regions_are_safe() {
    let mut doc = Document::new(16, 16);
    assert!(
        doc.composite_region(PixelRect {
            x0: 100,
            y0: 100,
            x1: 200,
            y1: 200
        })
        .is_empty()
    );
    let before = doc.clone();
    let mut stroke = Stroke::begin(
        &mut doc,
        Brush::default(),
        Point {
            x: -100.,
            y: -100.,
            pressure: 1.,
        },
    );
    assert!(stroke.take_damage().is_none());
    assert_eq!(doc, before);
}

#[test]
fn every_blend_mode_matches_partial_compositing_and_history() {
    use parfait_core::{BlendMode, History};
    for mode in BlendMode::ALL {
        let mut d = Document::new(4, 3);
        d.layers[0].locked = false;
        for (i, p) in Arc::make_mut(&mut d.layers[0].pixels)
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            p.copy_from_slice(&[40, 120, 230, (i * 23) as u8]);
        }
        for p in Arc::make_mut(&mut d.layers[1].pixels)
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
        {
            p.copy_from_slice(&[210, 90, 50, 180]);
        }
        d.layers[1].opacity = 0.6;
        let before = d.composite();
        let mut history = History::default();
        history.checkpoint(&d);
        d.layers[1].blend_mode = mode;
        d.touch();
        let full = d.composite();
        let region = d.composite_region(PixelRect {
            x0: 1,
            y0: 1,
            x1: 3,
            y1: 3,
        });
        assert_eq!(region, [&full[20..28], &full[36..44]].concat(), "{mode:?}");
        history.undo(&mut d);
        assert_eq!(d.composite(), before);
        history.redo(&mut d);
        assert_eq!(d.layers[1].blend_mode, mode);
        assert_eq!(d.composite(), full);
    }
}
