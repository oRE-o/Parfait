use parfait_core::{BlendMode, Brush, Document, History, LayerCombine, PixelRect, Point, Stroke};
use std::sync::Arc;

fn sample() -> Document {
    let mut document = Document::new(3, 1);
    document.layers.remove(0);
    document.active = 0;
    document.layers[0].pixels = Arc::new(vec![80, 120, 200, 128, 80, 120, 200, 128, 0, 0, 0, 0]);
    document.add_layer();
    document.layers[1].pixels = Arc::new(vec![255, 32, 48, 255, 0, 0, 0, 0, 255, 32, 48, 255]);
    document.layers[1].clipped = true;
    document.add_layer();
    document.layers[2].pixels = Arc::new(vec![0, 0, 0, 0, 16, 220, 64, 255, 16, 220, 64, 255]);
    document.layers[2].clipped = true;
    document
}

#[test]
fn chain_preserves_base_alpha_and_original_pixels_outside_the_base() {
    let mut document = sample();
    let original = document.clone();
    assert_eq!(
        document.composite(),
        [255, 32, 48, 128, 16, 220, 64, 128, 0, 0, 0, 0]
    );
    assert_eq!(document.layer_at(0, 0), Some(1));
    assert_eq!(document.layer_at(1, 0), Some(2));
    assert_eq!(document.layer_at(2, 0), None);
    assert_eq!(document.clipping_base(2), Some(0));
    assert_eq!(document.clipping_base(3), None);
    document.layers[0].opacity = 0.49;
    assert_eq!(
        document.composite(),
        [255, 32, 48, 63, 16, 220, 64, 63, 0, 0, 0, 0]
    );
    for (layer, before) in document.layers.iter().zip(&original.layers) {
        assert!(Arc::ptr_eq(&layer.pixels, &before.pixels));
    }
    document.layers[2].clipped = false;
    assert_eq!(&document.composite()[8..], &[16, 220, 64, 255]);
    assert!(!document.same_edit_state(&original));
}

#[test]
fn partial_clip_alpha_and_blend_modify_color_without_changing_base_coverage() {
    let mut document = sample();
    document.layers.truncate(2);
    document.active = 1;
    Arc::make_mut(&mut document.layers[1].pixels)[3] = 128;
    assert_eq!(&document.composite()[..4], &[168, 76, 124, 128]);
    document.layers[1].blend_mode = BlendMode::Multiply;
    assert_eq!(&document.composite()[..4], &[80, 67, 119, 128]);
    document.layers[0].opacity = 0.25;
    assert_eq!(&document.composite()[..4], &[80, 67, 119, 32]);
    for mode in BlendMode::ALL {
        document.layers[1].blend_mode = mode;
        assert_eq!(document.composite()[3], 32);
    }
    document.layers[1].opacity = 0.;
    assert_eq!(&document.composite()[..4], &[80, 120, 200, 32]);
}

#[test]
fn hidden_zero_opacity_and_orphan_bases_do_not_expose_clipped_pixels() {
    let original = sample();
    for variant in 0..4 {
        let mut document = original.clone();
        match variant {
            0 => document.layers[0].visible = false,
            1 => document.layers[0].opacity = 0.,
            2 => document.layers[0].clipped = true,
            _ => {
                document.layers.remove(0);
                document.active = 1;
            }
        }
        assert!(document.validate());
        assert!(document.composite().iter().all(|byte| *byte == 0));
        assert_eq!(document.layer_at(0, 0), None);
        assert_eq!(document.layer_at(1, 0), None);
    }
    let mut document = original;
    document.layers[1].visible = false;
    assert_eq!(document.clipping_base(2), Some(0));
    assert_eq!(&document.composite()[4..8], &[16, 220, 64, 128]);
    document.layers.swap(0, 1);
    assert_eq!(document.clipping_base(0), None);
    assert_eq!(document.clipping_base(2), Some(1));
}

#[test]
fn insertion_inherits_clipping_and_history_restores_flags_and_shared_pixels() {
    let mut document = sample();
    document.active = 0;
    document.add_layer();
    assert!(document.layers[1].clipped);
    assert_eq!(document.clipping_base(3), Some(0));
    let before = document.clone();
    let mut history = History::default();
    history.begin(&document, "Clip to layer below");
    document.layers[2].clipped = false;
    document.touch();
    assert!(history.commit(&document));
    assert!(history.undo(&mut document));
    assert!(document.same_edit_state(&before));
    assert!(history.redo(&mut document));
    assert!(!document.layers[2].clipped);
    assert!(Arc::ptr_eq(
        &document.layers[2].pixels,
        &before.layers[2].pixels
    ));
}

#[test]
fn clipping_does_not_restrict_raw_brush_writes() {
    let mut document = Document::new(16, 16);
    document.layers[0].visible = false;
    document.add_layer();
    document.layers[2].clipped = true;
    let _stroke = Stroke::begin(
        &mut document,
        Brush {
            size: 6.,
            pressure: false,
            color: [255, 0, 0],
            ..Default::default()
        },
        Point {
            x: 8.,
            y: 8.,
            pressure: 1.,
        },
    );
    assert!(
        document.layers[2]
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] > 0)
    );
    assert!(document.composite().iter().all(|byte| *byte == 0));
    document.layers[2].clipped = false;
    assert!(
        document
            .composite()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] > 0)
    );
}

#[test]
fn base_blend_does_not_reapply_to_normal_clipping_color() {
    let mut document = sample();
    let mut backdrop = document.layers[0].clone();
    backdrop.id = document.allocate_layer_id().unwrap();
    backdrop.pixels = Arc::new([96, 192, 224, 255].repeat(3));
    document.layers.insert(0, backdrop);
    document.layers[1].opacity = 0.98;
    for mode in BlendMode::ALL {
        document.layers[1].blend_mode = mode;
        let composite = document.composite();
        assert_eq!(&composite[..4], &[174, 113, 137, 255]);
        assert_eq!(&composite[4..8], &[57, 206, 145, 255]);
        assert_eq!(&composite[8..], &[96, 192, 224, 255]);
    }
}

#[test]
fn complete_chain_combinations_bake_the_display_and_incomplete_subsets_are_atomic() {
    for action in [
        LayerCombine::MergeVisible,
        LayerCombine::Flatten,
        LayerCombine::CopyVisible,
        LayerCombine::MergeSelected,
    ] {
        for mode in BlendMode::ALL {
            let mut document = sample();
            document.layers[2].blend_mode = mode;
            let expected = document.composite();
            if action == LayerCombine::MergeSelected && mode != BlendMode::Normal {
                continue;
            }
            assert!(document.combine_layers(action, 7), "{action:?} {mode:?}");
            if action == LayerCombine::CopyVisible {
                assert_eq!(*document.layers.last().unwrap().pixels, expected);
            } else {
                assert_eq!(document.composite(), expected);
            }
            assert!(!document.layers.last().unwrap().clipped);
        }
    }
    for (action, mask, active) in [
        (LayerCombine::MergeDown, 4, 2),
        (LayerCombine::MergeDown, 2, 1),
        (LayerCombine::TransferDown, 2, 1),
        (LayerCombine::MergeSelected, 6, 2),
    ] {
        let mut document = sample();
        document.active = active;
        let before = document.clone();
        assert!(
            document
                .layer_combine_reason(action, mask)
                .unwrap()
                .contains("clipping")
        );
        assert!(!document.combine_layers(action, mask));
        assert_eq!(document, before);
    }
    let mut document = sample();
    document.layers.pop();
    document.active = 1;
    let expected = document.composite();
    assert!(document.combine_layers(LayerCombine::TransferDown, 2));
    assert_eq!(document.composite(), expected);
    assert!(document.layers[1].clipped);
}

#[test]
fn partial_composition_matches_full_for_every_mode_and_sixty_four_layer_chains() {
    let mut document = Document::new(13, 9);
    while document.layers.len() < 64 {
        document.add_layer();
    }
    for (index, layer) in document.layers.iter_mut().enumerate() {
        layer.clipped = index % 5 != 0;
        layer.visible = index % 7 != 0;
        layer.opacity = (index % 9) as f32 / 8.;
        for (offset, pixel) in Arc::make_mut(&mut layer.pixels)
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            *pixel = [
                (offset * 31 + index * 7) as u8,
                (offset * 19) as u8,
                (index * 29) as u8,
                (offset * 53 + index * 13) as u8,
            ];
        }
    }
    for mode in BlendMode::ALL {
        for layer in &mut document.layers {
            layer.blend_mode = mode;
        }
        let full = document.composite();
        for rect in [
            PixelRect {
                x0: 2,
                y0: 3,
                x1: 12,
                y1: 8,
            },
            PixelRect {
                x0: 12,
                y0: 8,
                x1: 90,
                y1: 90,
            },
            PixelRect {
                x0: 50,
                y0: 30,
                x1: 70,
                y1: 80,
            },
        ] {
            let clipped = rect.clipped(document.width, document.height);
            let expected: Vec<_> = (clipped.y0..clipped.y1)
                .flat_map(|row| {
                    let start = ((row * document.width + clipped.x0) * 4) as usize;
                    full[start..start + ((clipped.x1 - clipped.x0) * 4) as usize]
                        .iter()
                        .copied()
                })
                .collect();
            assert_eq!(document.composite_region(rect), expected, "{mode:?}");
        }
    }
}
