use parfait_core::{Brush, BrushTip, Document, PixelRect, Point, Stroke};
use std::sync::Arc;

#[test]
fn incremental_pixels_match_full_composite_for_all_tips_and_eraser() {
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
        for erase in [false, true] {
            let mut doc = Document::new(96, 80);
            doc.layers[0].visible = false;
            doc.layers[1].opacity = 0.63;
            Arc::make_mut(&mut doc.layers[1].pixels).fill(120);
            let mut cached = doc.composite();
            let brush = Brush {
                tip: tip.clone(),
                erase,
                size: 27.,
                angle: 37.,
                aspect: 0.5,
                pressure_opacity: true,
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
