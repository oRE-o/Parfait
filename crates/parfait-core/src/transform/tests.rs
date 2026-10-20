use super::*;
fn sample_doc() -> Document {
    let mut doc = Document::new(6, 4);
    let pixels = Arc::make_mut(&mut doc.layers[1].pixels);
    for y in 0..4 {
        for x in 0..6 {
            let i = (y * 6 + x) * 4;
            pixels[i..i + 4].copy_from_slice(&[
                x as u8 * 30,
                y as u8 * 40,
                80,
                255,
            ]);
        }
    }
    doc
}
#[test]
fn identity_all_warp_modes_is_a_true_noop() {
    for kind in [WarpKind::Projective, WarpKind::Mesh, WarpKind::Pins] {
        let mut doc = sample_doc();
        let before = doc.clone();
        let t = RasterTransform { kind, ..Default::default() };
        assert!(!doc.transform_layer_pixels(1, &t));
        assert_eq!(doc, before);
        assert!(Arc::ptr_eq(&doc.layers[1].pixels, &before.layers[1].pixels));
    }
}
#[test]
fn exact_flips_and_integer_translation_preserve_pixel_values_and_linked_masks()
{
    let mut doc = sample_doc();
    doc.create_layer_mask(1, 255);
    doc.layers[1].mask.as_mut().unwrap().coverage =
        MaskCoverage::Dense(Arc::new((0..24).map(|i| i * 10).collect()));
    let before = doc.clone();
    let t = RasterTransform { scale: [-1., 1.], ..Default::default() };
    assert!(doc.transform_layer_pixels(1, &t));
    assert!(doc.validate());
    for y in 0..4 {
        for x in 0..6 {
            let i = y * 6 + x;
            let src = y * 6 + 5 - x;
            assert_eq!(
                &doc.layers[1].pixels[i * 4..i * 4 + 4],
                &before.layers[1].pixels[src * 4..src * 4 + 4]
            );
            assert_eq!(
                doc.layers[1].mask.as_ref().unwrap().coverage.value(i),
                before.layers[1].mask.as_ref().unwrap().coverage.value(src)
            );
        }
    }
    let mut doc = sample_doc();
    let before = doc.clone();
    let t = RasterTransform { translation: [1., 0.], ..Default::default() };
    assert!(doc.transform_layer_pixels(1, &t));
    assert_eq!(&doc.layers[1].pixels[4..8], &before.layers[1].pixels[..4]);
    assert_eq!(&doc.layers[1].pixels[..4], &[0; 4]);
}
#[test]
fn selection_transform_moves_only_selected_pixels_and_updates_selection() {
    let mut doc = sample_doc();
    let mut values = vec![0; 24];
    values[0] = 255;
    doc.selection = Some(MaskCoverage::Dense(Arc::new(values)));
    let before = doc.clone();
    let t = RasterTransform { translation: [1., 0.], ..Default::default() };
    assert!(doc.transform_layer_pixels(1, &t));
    assert!(doc.validate());
    assert_eq!(&doc.layers[1].pixels[..4], &[0; 4]);
    assert_eq!(&doc.layers[1].pixels[4..8], &before.layers[1].pixels[..4]);
    assert_eq!(&doc.layers[1].pixels[8..], &before.layers[1].pixels[8..]);
    assert_eq!(doc.selection_value(0), 0);
    assert_eq!(doc.selection_value(1), 255);
}
#[test]
fn projective_mesh_and_pin_warps_make_finite_valid_edits_and_bad_meshes_are_atomic()
 {
    for t in [
        RasterTransform { scale: [1.5, 0.8], angle: 17., ..Default::default() },
        RasterTransform { shear: [0.3, 0.], ..Default::default() },
        RasterTransform { perspective: [0.4, -0.2], ..Default::default() },
        RasterTransform {
            kind: WarpKind::Mesh,
            nodes: (0..9)
                .map(|i| if i == 4 { [0.12, 0.1] } else { [0.; 2] })
                .collect(),
            ..Default::default()
        },
        RasterTransform {
            kind: WarpKind::Pins,
            pins: vec![
                WarpPin { source: [0., 0.], target: [0., 0.], rotation: 0. },
                WarpPin { source: [1., 1.], target: [1., 1.], rotation: 0. },
                WarpPin {
                    source: [0.5, 0.5],
                    target: [0.7, 0.6],
                    rotation: 0.,
                },
            ],
            ..Default::default()
        },
    ] {
        let mut doc = sample_doc();
        assert!(doc.transform_layer_pixels(1, &t), "{t:?}");
        assert!(doc.validate());
    }
    for t in [
        RasterTransform { scale: [0., 1.], ..Default::default() },
        RasterTransform { angle: f32::NAN, ..Default::default() },
        RasterTransform {
            kind: WarpKind::Mesh,
            nodes: (0..9)
                .map(|i| if i == 4 { [2., 2.] } else { [0.; 2] })
                .collect(),
            ..Default::default()
        },
    ] {
        let mut doc = sample_doc();
        let before = doc.clone();
        assert!(!doc.transform_layer_pixels(1, &t));
        assert_eq!(doc, before);
    }
}

#[test]
fn arbitrary_pins_hold_exact_positions_and_single_pin_rotation_is_rigid() {
    let bounds = PixelRect { x0: 0, y0: 0, x1: 100, y1: 100 };
    let t = RasterTransform {
        kind: WarpKind::Pins,
        pins: vec![
            WarpPin {
                source: [0.23, 0.41],
                target: [0.31, 0.44],
                rotation: 0.,
            },
            WarpPin {
                source: [0.82, 0.77],
                target: [0.82, 0.77],
                rotation: 0.,
            },
        ],
        ..Default::default()
    };
    assert!(t.validate());
    for pin in &t.pins {
        let source = t
            .unmap_point([pin.target[0] * 100., pin.target[1] * 100.], bounds)
            .unwrap();
        assert!((source[0] - pin.source[0] * 100.).abs() < 0.001);
        assert!((source[1] - pin.source[1] * 100.).abs() < 0.001);
    }
    let t = RasterTransform {
        kind: WarpKind::Pins,
        pins: vec![WarpPin {
            source: [0.5, 0.5],
            target: [0.5, 0.5],
            rotation: 90.,
        }],
        ..Default::default()
    };
    assert!(t.validate());
    let source = t.unmap_point([50., 75.], bounds).unwrap();
    assert!((source[0] - 75.).abs() < 0.001 && (source[1] - 50.).abs() < 0.001);
}
#[test]
fn variable_mesh_identity_quad_fitting_cancel_and_alpha_lock_are_safe() {
    for (columns, rows) in [(2, 2), (4, 5), (16, 16)] {
        let mut t =
            RasterTransform { kind: WarpKind::Mesh, ..Default::default() };
        assert!(t.resize_grid(columns, rows));
        assert!(t.validate());
        let mut d = sample_doc();
        let before = d.clone();
        assert!(!d.transform_layer_pixels(1, &t));
        assert!(d.same_edit_state(&before));
    }
    let bounds = PixelRect { x0: 0, y0: 0, x1: 100, y1: 80 };
    let quad = [[2., 3.], [105., 7.], [96., 88.], [0., 80.]];
    let t = RasterTransform::fit_quad(bounds, quad).unwrap();
    for (source, destination) in
        [[0., 0.], [100., 0.], [100., 80.], [0., 80.]].into_iter().zip(quad)
    {
        let mapped = t.map_point(source, bounds).unwrap();
        assert!(distance(mapped, destination) < 0.0001);
        let recovered = t.unmap_point(destination, bounds).unwrap();
        assert!(distance(source, recovered) < 0.0001);
    }
    let t = RasterTransform { translation: [1., 0.], ..Default::default() };
    let mut d = sample_doc();
    let before = d.clone();
    assert!(!d.transform_layer_pixels_cancellable(1, &t, || true));
    assert!(d.same_edit_state(&before));
    d.layers[1].alpha_locked = true;
    assert!(d.transform_reason(1, &t).is_some());
}
#[test]
fn mask_transform_preserves_content_and_empty_exterior_and_undo() {
    let mut d = sample_doc();
    d.create_layer_mask(1, 255);
    let mut mask = vec![255; 24];
    mask[7] = 0;
    mask[8] = 128;
    d.layers[1].mask.as_mut().unwrap().coverage =
        MaskCoverage::Dense(Arc::new(mask));
    let before = d.clone();
    let mut history = crate::History::default();
    history.begin(&d, "Mask transform");
    let t = RasterTransform { translation: [1., 0.], ..Default::default() };
    assert!(d.transform_layer_mask(1, &t));
    assert!(Arc::ptr_eq(&before.layers[1].pixels, &d.layers[1].pixels));
    let coverage = &d.layers[1].mask.as_ref().unwrap().coverage;
    assert_eq!(coverage.value(7), 255);
    assert_eq!(coverage.value(8), 0);
    assert_eq!(coverage.value(9), 128);
    history.commit(&d);
    assert!(history.undo(&mut d));
    assert!(d.same_edit_state(&before));
    let before = d.clone();
    assert!(!d.transform_layer_mask_cancellable(1, &t, || true));
    assert!(d.same_edit_state(&before));
}
