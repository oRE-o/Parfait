use super::*;
use crate::{LayerKind, MaskCoverage};
fn fixture() -> Document {
    let mut d = Document::new(3, 1);
    d.layers[1].pixels =
        Arc::new(vec![24, 80, 160, 255, 128, 128, 128, 128, 220, 140, 40, 0]);
    d
}
#[test]
fn identity_luts_are_exact_including_hidden_rgb_and_shared_buffers() {
    for edit in [
        ToneAdjustment::Levels([ToneLevels::default(); 4]),
        ToneAdjustment::Curves(std::array::from_fn(|_| {
            ResponseCurve::default()
        })),
        ToneAdjustment::Hsv { hue: 0., saturation: 0., value: 0. },
        ToneAdjustment::Balance {
            ranges: [[0.; 3]; 3],
            preserve_luminance: true,
        },
    ] {
        let mut d = fixture();
        let before = d.clone();
        assert!(!d.adjust_layer_tones(2, &edit), "{edit:?}");
        assert_eq!(d, before);
        assert!(Arc::ptr_eq(&d.layers[1].pixels, &before.layers[1].pixels));
    }
}
#[test]
fn channels_output_levels_and_selection_are_applied_exactly_once() {
    let mut d = fixture();
    d.selection = Some(MaskCoverage::Dense(Arc::new(vec![255, 128, 0])));
    let before = d.clone();
    let mut levels = [ToneLevels::default(); 4];
    levels[1].output_black = 255;
    levels[1].output_white = 0;
    assert!(d.adjust_layer_tones(2, &ToneAdjustment::Levels(levels)));
    assert_eq!(&d.layers[1].pixels[..4], &[231, 80, 160, 255]);
    assert_eq!(
        &d.layers[1].pixels[4..8],
        &crate::selection::interpolate_pixel(
            [128; 4],
            [127, 128, 128, 128],
            128
        )
    );
    assert_eq!(&d.layers[1].pixels[8..], &before.layers[1].pixels[8..]);
}
#[test]
fn multistop_gradient_nodes_reverse_and_linear_light_are_correct() {
    let stops = vec![
        GradientStop { position: 0., color: [0; 3], midpoint: 0.5 },
        GradientStop { position: 0.5, color: [255, 0, 0], midpoint: 0.5 },
        GradientStop { position: 1., color: [255; 3], midpoint: 0.5 },
    ];
    let mut edit =
        ToneAdjustment::Gradient { stops, reverse: false, linear_light: false };
    assert!(edit.validate());
    assert_eq!(edit.gradient_color(0.5), [255, 0, 0]);
    assert_eq!(edit.gradient_color(0.25), [128, 0, 0]);
    if let ToneAdjustment::Gradient { reverse, .. } = &mut edit {
        *reverse = true;
    }
    assert_eq!(edit.gradient_color(0.25), [255, 128, 128]);
    if let ToneAdjustment::Gradient { reverse, linear_light, .. } = &mut edit {
        *reverse = false;
        *linear_light = true;
    }
    assert_eq!(edit.gradient_color(0.25), [188, 0, 0]);
}
#[test]
fn smooth_arbitrary_curve_can_invert_without_overshooting() {
    let mut curves = std::array::from_fn(|_| ResponseCurve::default());
    curves[0].points = vec![[0., 1.], [0.4, 0.6], [1., 0.]];
    let mut d = fixture();
    assert!(d.adjust_layer_tones(2, &ToneAdjustment::Curves(curves)));
    assert_eq!(&d.layers[1].pixels[..4], &[231, 175, 95, 255]);
}
#[test]
fn balance_ranges_and_luminance_are_independent() {
    let ranges = [[0.5, 0., 0.], [0., 0., 0.], [0., 0., 0.]];
    let mut dark = Document::new(2, 1);
    dark.layers[1].pixels = Arc::new(vec![32, 32, 32, 255, 240, 240, 240, 255]);
    let original = dark.clone();
    assert!(dark.adjust_layer_tones(
        2,
        &ToneAdjustment::Balance { ranges, preserve_luminance: false }
    ));
    assert!(dark.layers[1].pixels[0] > original.layers[1].pixels[0]);
    assert_eq!(&dark.layers[1].pixels[4..], &original.layers[1].pixels[4..]);
    let mut kept = original.clone();
    assert!(kept.adjust_layer_tones(
        2,
        &ToneAdjustment::Balance { ranges, preserve_luminance: true }
    ));
    let p = &kept.layers[1].pixels;
    assert!((luma([p[0], p[1], p[2]].map(f32::from)) - 32.).abs() < 0.6);
}
#[test]
fn invalid_or_locked_multilayer_edit_and_cancellation_never_partially_apply() {
    let mut d = fixture();
    d.add_layer();
    d.layers[2].pixels = d.layers[1].pixels.clone();
    let mut levels = [ToneLevels::default(); 4];
    levels[0].output_black = 80;
    let edit = ToneAdjustment::Levels(levels);
    d.layers[2].locked = true;
    let before = d.clone();
    assert!(!d.adjust_layer_tones(6, &edit));
    assert_eq!(d, before);
    d.layers[2].locked = false;
    let before = d.clone();
    let counter = std::cell::Cell::new(0);
    assert!(!d.adjust_layer_tones_cancellable(6, &edit, || {
        counter.set(counter.get() + 1);
        counter.get() > 1
    }));
    assert_eq!(d, before);
    assert!(d.adjust_layer_tones(6, &edit));
    assert_eq!(d.layers[1].pixels, d.layers[2].pixels);
    d.layers[2].kind = LayerKind::Folder;
    assert!(d.tone_edit_reason(6, &edit).is_some());
}
#[test]
fn histogram_ignores_transparent_unselected_and_masked_pixels() {
    let mut d = fixture();
    d.selection = Some(MaskCoverage::Dense(Arc::new(vec![255, 0, 255])));
    let bins = d.tone_histogram(2);
    assert_eq!(bins[1].iter().sum::<u64>(), 65025);
    assert_eq!(bins[1][24], 65025);
    assert_eq!(automatic_tone_range(&bins[1]), (0, 255));
    d.create_layer_mask(1, 0);
    assert!(d.tone_histogram(2).iter().flatten().all(|n| *n == 0));
}
#[test]
fn hsv_value_and_threshold_transparency_have_explicit_semantics() {
    let mut d = fixture();
    assert!(d.adjust_layer_tones(
        2,
        &ToneAdjustment::Hsv { hue: 120., saturation: 0., value: 0. }
    ));
    assert_eq!(&d.layers[1].pixels[..4], &[160, 24, 80, 255]);
    let mut d = fixture();
    d.layers[1].alpha_locked = true;
    let edit = ToneAdjustment::Threshold { value: 100, preserve_alpha: false };
    assert!(d.tone_edit_reason(2, &edit).is_some());
    d.layers[1].alpha_locked = false;
    assert!(d.adjust_layer_tones(2, &edit));
    assert_eq!(d.layers[1].pixels[7], 255);
    assert_eq!(d.layers[1].pixels[11], 0);
}
#[test]
fn hsv_preserves_neutral_colors_and_lightness_reaches_white_or_black() {
    let mut gray = Document::new(256, 1);
    gray.layers[1].pixels =
        Arc::new((0..=255).flat_map(|v| [v, v, v, 128]).collect());
    for saturation in [-1., 0., 0.7, 1.] {
        let mut d = gray.clone();
        assert!(!d.adjust_layer_tones(
            2,
            &ToneAdjustment::Hsv { hue: 120., saturation, value: 0. }
        ));
        assert_eq!(d, gray);
    }
    for (value, expected) in [(1., 255), (-1., 0)] {
        let mut d = fixture();
        assert!(d.adjust_layer_tones(
            2,
            &ToneAdjustment::Hsv { hue: 90., saturation: 0.5, value }
        ));
        assert_eq!(
            &d.layers[1].pixels[..4],
            &[expected, expected, expected, 255]
        );
        assert_eq!(
            &d.layers[1].pixels[4..8],
            &[expected, expected, expected, 128]
        );
    }
}
#[test]
fn malformed_curves_levels_and_gradients_are_rejected() {
    let mut levels = [ToneLevels::default(); 4];
    levels[0].gamma = f32::NAN;
    assert!(!ToneAdjustment::Levels(levels).validate());
    let stops =
        vec![GradientStop { position: 0., color: [0; 3], midpoint: 0.5 }; 2];
    assert!(
        !ToneAdjustment::Gradient {
            stops,
            reverse: false,
            linear_light: false
        }
        .validate()
    );
}
#[test]
fn prepared_gradient_is_pixel_exact_with_scalar_sampling() {
    for linear_light in [false, true] {
        for reverse in [false, true] {
            let edit = ToneAdjustment::Gradient {
                stops: vec![
                    GradientStop {
                        position: 0.,
                        color: [25, 12, 61],
                        midpoint: 0.45,
                    },
                    GradientStop {
                        position: 0.48,
                        color: [180, 72, 94],
                        midpoint: 0.6,
                    },
                    GradientStop {
                        position: 1.,
                        color: [255, 234, 185],
                        midpoint: 0.5,
                    },
                ],
                reverse,
                linear_light,
            };
            let prepared = PreparedGradient::new(&edit).unwrap();
            for r in (0..=255).step_by(7) {
                for g in (0..=255).step_by(11) {
                    for b in (0..=255).step_by(13) {
                        let light = luma([r, g, b].map(|v| v as f32 / 255.));
                        assert_eq!(
                            prepared.color(light),
                            edit.gradient_color(light)
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn nearest_resize_preserves_palette_and_remaps_masks_and_selection() {
    let mut d = Document::new(2, 1);
    d.layers[1].pixels = Arc::new(vec![255, 0, 0, 128, 0, 0, 255, 255]);
    d.selection = Some(MaskCoverage::Dense(Arc::new(vec![17, 230])));
    d.create_layer_mask(1, 90);
    assert!(
        d.edit_canvas(crate::CanvasEdit::ResizeNearest { width: 4, height: 2 })
    );
    assert_eq!(
        &d.layers[1].pixels[..16],
        &[255, 0, 0, 128, 255, 0, 0, 128, 0, 0, 255, 255, 0, 0, 255, 255]
    );
    assert_eq!(
        (0..4).map(|i| d.selection_value(i)).collect::<Vec<_>>(),
        vec![17, 17, 230, 230]
    );
    assert!(d.validate());
}
