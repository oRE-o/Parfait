use super::*;
use crate::{BrushTip, History, Stroke};

fn point() -> Point {
    Point { x: 16.5, y: 16.5, pressure: 1. }
}

fn document(coverage: u8) -> Document {
    let mut document = Document::new(64, 48);
    document.create_layer_mask(1, coverage);
    document
}

fn brush() -> Brush {
    Brush { size: 8., pressure: false, smoothing: 0., ..Default::default() }
}

fn center(document: &Document) -> u8 {
    document.layers[1].mask.as_ref().unwrap().coverage.value(16 * 64 + 16)
}

#[test]
fn scalar_deposition_is_color_independent_and_never_writes_content() {
    for initial in [0, 1, 127, 128, 254, 255] {
        for erase in [false, true] {
            for opacity in [0., 0.1, 0.5, 1.] {
                let mut document = document(initial);
                let before = document.clone();
                let options = Brush { opacity, erase, ..brush() };
                let mut alternate = document.clone();
                Stroke::begin_mask(&mut document, options.clone(), point())
                    .unwrap();
                Stroke::begin_mask(
                    &mut alternate,
                    Brush { color: [255, 255, 255], ..options },
                    point(),
                )
                .unwrap();
                let expected = if erase {
                    (f32::from(initial) * (1. - opacity)).round() as u8
                } else {
                    (f32::from(initial) + f32::from(255 - initial) * opacity)
                        .round() as u8
                };
                assert_eq!(center(&document), expected);
                assert_eq!(document.layers, alternate.layers);
                for (current, previous) in
                    document.layers.iter().zip(&before.layers)
                {
                    assert!(Arc::ptr_eq(&current.pixels, &previous.pixels));
                }
                if expected == initial {
                    assert_eq!(document.revision, before.revision);
                    assert!(matches!(
                        document.layers[1].mask.as_ref().unwrap().coverage,
                        MaskCoverage::Uniform(_)
                    ));
                }
            }
        }
    }
}

#[test]
fn capped_mask_strokes_share_spacing_pressure_spray_and_independent_baselines()
{
    for erase in [false, true] {
        let initial = if erase { 255 } else { 0 };
        let mut document = document(initial);
        document.layers[1].alpha_locked = true;
        let options = Brush {
            erase,
            opacity: 0.4,
            flow: 0.25,
            opacity_mode: OpacityMode::PerStroke,
            continuous_spray: true,
            spray_rate: 120.,
            ..brush()
        };
        let mut stroke =
            Stroke::begin_mask(&mut document, options.clone(), point())
                .unwrap();
        assert_eq!(center(&document), if erase { 230 } else { 26 });
        for _ in 0..40 {
            stroke.spray(&mut document, 0.1, 1.);
        }
        assert_eq!(center(&document), if erase { 153 } else { 102 });
        assert!(
            stroke.scratch_capacity_bytes()
                <= 16_384 + std::mem::size_of::<usize>()
        );
        let revision = document.revision;
        stroke.spray(&mut document, 0.1, 1.);
        assert_eq!(document.revision, revision);
        let mut stroke =
            Stroke::begin_mask(&mut document, options, point()).unwrap();
        for _ in 0..40 {
            stroke.spray(&mut document, 0.1, 1.);
        }
        assert_eq!(center(&document), if erase { 92 } else { 163 });
    }
    let mut low = document(0);
    let mut high = low.clone();
    let options = Brush { pressure_opacity: true, ..brush() };
    Stroke::begin_mask(
        &mut low,
        options.clone(),
        Point { pressure: 0.1, ..point() },
    )
    .unwrap();
    Stroke::begin_mask(&mut high, options, Point { pressure: 0.8, ..point() })
        .unwrap();
    assert!(center(&high) > center(&low));
}

#[test]
fn mask_strokes_keep_noop_dense_identity_and_history_future() {
    for mode in [OpacityMode::PerDab, OpacityMode::PerStroke] {
        for case in 0..4 {
            let mut document = document(127);
            document.layers[1].mask.as_mut().unwrap().coverage =
                MaskCoverage::Dense(Arc::new(vec![127; 64 * 48]));
            let mut history = History::default();
            history.begin(&document, "Other edit");
            document.layers[1].opacity = 0.8;
            history.commit(&document);
            history.undo(&mut document);
            let before = document.clone();
            let mut options = Brush { opacity_mode: mode, ..brush() };
            let mut location = point();
            match case {
                0 => options.opacity = 0.,
                1 => {
                    options.pressure_opacity = true;
                    location.pressure = 0.;
                }
                2 => location.x = -100.,
                _ => {
                    options.tip = BrushTip::Image {
                        width: 1,
                        height: 1,
                        mask: Arc::new(vec![0]),
                    }
                }
            }
            history.begin(&document, "Mask");
            let stroke =
                Stroke::begin_mask(&mut document, options, location).unwrap();
            assert_eq!(stroke.scratch_capacity_bytes(), 0);
            assert!(!history.commit(&document));
            assert!(history.can_redo());
            assert_eq!(document.revision, before.revision);
            assert!(document.same_edit_state(&before));
        }
    }
}

#[test]
fn mask_guards_reject_unsupported_targets_without_touching_content() {
    for case in 0..9 {
        let mut document = document(127);
        let mut options = brush();
        match case {
            0 => document.layers[1].mask = None,
            1 => document.layers[1].mask.as_mut().unwrap().enabled = false,
            2 => document.layers[1].locked = true,
            3 => document.layers[1].visible = false,
            4 => options.operation = BrushOperation::Blend,
            5 => options.operation = BrushOperation::Blur,
            6 => options.operation = BrushOperation::Smudge,
            7 => {
                document.layers[1].mask.as_mut().unwrap().coverage =
                    MaskCoverage::Dense(Arc::new(vec![0]))
            }
            _ => options.size = f32::NAN,
        }
        let before = document.clone();
        assert!(Stroke::begin_mask(&mut document, options, point()).is_none());
        assert_eq!(document, before);
    }
}

#[test]
fn mask_stroke_damage_history_and_region_output_match_full_composition() {
    for mode in [OpacityMode::PerDab, OpacityMode::PerStroke] {
        let mut document = document(255);
        document.layers[1].pixels =
            Arc::new([220, 70, 20, 255].repeat(64 * 48));
        document.layers[1].clipped = true;
        let before = document.clone();
        let mut history = History::default();
        history.begin(&document, "Mask brush");
        let mut stroke = Stroke::begin_mask(
            &mut document,
            Brush { erase: true, opacity_mode: mode, ..brush() },
            point(),
        )
        .unwrap();
        stroke.extend(&mut document, Point { x: 50., ..point() });
        let damage = stroke.take_damage().unwrap();
        let full = document.composite();
        let region = document.composite_region(damage);
        for row in damage.y0..damage.y1 {
            let start = ((row * document.width + damage.x0) * 4) as usize;
            let local =
                ((row - damage.y0) * (damage.x1 - damage.x0) * 4) as usize;
            assert_eq!(
                &region[local..local + (damage.x1 - damage.x0) as usize * 4],
                &full[start..start + (damage.x1 - damage.x0) as usize * 4]
            );
        }
        assert!(history.commit(&document));
        let after = document.clone();
        history.undo(&mut document);
        assert!(document.same_edit_state(&before));
        history.redo(&mut document);
        assert!(document.same_edit_state(&after));
        assert!(Arc::ptr_eq(
            &document.layers[1].pixels,
            &before.layers[1].pixels
        ));
    }
}

#[test]
fn split_mask_input_retains_dab_positions_and_dense_snapshot_independence() {
    let render = |pieces| {
        let mut document = document(0);
        let mut stroke = Stroke::begin_mask(
            &mut document,
            Brush { flow: 0.3, spacing: 0.25, ..brush() },
            point(),
        )
        .unwrap();
        for step in 1..=pieces {
            stroke.extend(
                &mut document,
                Point {
                    x: 16.5 + 32. * step as f32 / pieces as f32,
                    ..point()
                },
            );
        }
        document
    };
    let mut single = render(1);
    for pieces in [2, 8, 64] {
        assert_eq!(single.layers, render(pieces).layers);
    }
    let before = single.clone();
    Stroke::begin_mask(&mut single, Brush { erase: true, ..brush() }, point())
        .unwrap();
    assert_ne!(single.layers[1].mask, before.layers[1].mask);
    assert!(Arc::ptr_eq(&single.layers[1].pixels, &before.layers[1].pixels));
}
