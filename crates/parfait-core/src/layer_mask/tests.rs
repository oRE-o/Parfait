use super::*;
use crate::{BlendMode, History, LayerCombine, PixelRect};

#[test]
fn mask_clear_restores_coverage_without_touching_content_or_enable_state() {
    for dense in [false, true] {
        for enabled in [false, true] {
            let mut document = Document::new(8, 8);
            document.create_layer_mask(1, 127);
            document.layers[1].alpha_locked = true;
            let mask = document.layers[1].mask.as_mut().unwrap();
            mask.enabled = enabled;
            if dense {
                mask.coverage =
                    MaskCoverage::Dense(Arc::new((0..64).map(|index| index * 4).collect()));
            }
            let before = document.clone();
            let mut history = History::default();
            history.begin(&document, "Clear mask");
            assert!(document.clear_layer_mask(1));
            assert!(history.commit(&document));
            assert_eq!(
                document.layers[1].mask.as_ref().unwrap().coverage,
                MaskCoverage::Uniform(255)
            );
            assert_eq!(document.layers[1].mask.as_ref().unwrap().enabled, enabled);
            for (current, previous) in document.layers.iter().zip(&before.layers) {
                assert!(Arc::ptr_eq(&current.pixels, &previous.pixels));
            }
            let after = document.clone();
            history.undo(&mut document);
            assert!(document.same_edit_state(&before));
            history.redo(&mut document);
            assert!(document.same_edit_state(&after));
        }
    }
}

#[test]
fn mask_clear_noops_and_invalid_targets_preserve_buffers_revision_and_redo() {
    for case in 0..6 {
        let mut document = Document::new(8, 8);
        document.create_layer_mask(1, 255);
        match case {
            0 => {}
            1 => {
                document.layers[1].mask.as_mut().unwrap().coverage =
                    MaskCoverage::Dense(Arc::new(vec![255; 64]))
            }
            2 => {
                document.layers[1].locked = true;
                document.layers[1].mask.as_mut().unwrap().coverage = MaskCoverage::Uniform(0);
            }
            3 => document.layers[1].mask = None,
            4 => {
                document.layers[1].mask.as_mut().unwrap().coverage =
                    MaskCoverage::Dense(Arc::new(vec![0; 3]))
            }
            _ => {}
        }
        let mut history = History::default();
        history.begin(&document, "Opacity");
        document.layers[1].opacity = 0.5;
        history.commit(&document);
        history.undo(&mut document);
        let before = document.clone();
        history.begin(&document, "Clear mask");
        assert!(!document.clear_layer_mask(if case == 5 { 99 } else { 1 }));
        assert!(!history.commit(&document));
        assert_eq!(document.revision, before.revision);
        assert!(document.same_edit_state(&before));
        assert!(history.can_redo());
    }
}

fn sample(width: u32, height: u32, count: usize) -> Document {
    let mut document = Document::new(width, height);
    while document.layers.len() < count {
        document.add_layer();
    }
    document.layers.truncate(count);
    document.active = count - 1;
    for (index, layer) in document.layers.iter_mut().enumerate() {
        layer.locked = false;
        layer.clipped = index % 3 != 0;
        layer.opacity = 0.73;
        for (offset, pixel) in Arc::make_mut(&mut layer.pixels)
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            *pixel = [
                (offset * 17 + index * 71) as u8,
                (offset * 53) as u8,
                (index * 37) as u8,
                (offset * 29 + index * 43) as u8,
            ];
        }
        layer.mask = Some(LayerMask {
            enabled: index % 4 != 1,
            coverage: if index % 2 == 0 {
                MaskCoverage::Dense(Arc::new(
                    (0..width as usize * height as usize)
                        .map(|offset| (offset * 23 + index * 61) as u8)
                        .collect(),
                ))
            } else {
                MaskCoverage::Uniform(127)
            },
        });
    }
    document
}

fn independently_baked(document: &Document) -> Document {
    let mut baked = document.clone();
    for layer in &mut baked.layers {
        if let Some(mask) = layer.mask.take().filter(|mask| mask.enabled) {
            for (index, pixel) in Arc::make_mut(&mut layer.pixels)
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .enumerate()
            {
                let value = match &mask.coverage {
                    MaskCoverage::Uniform(value) => *value,
                    MaskCoverage::Dense(values) => values[index],
                };
                pixel[3] = ((u32::from(pixel[3]) * u32::from(value) + 127) / 255) as u8;
            }
        }
    }
    baked
}

#[test]
fn masked_composition_matches_baked_reference_all_modes_clipping_masks_and_regions() {
    for base_mode in BlendMode::ALL {
        for clip_mode in BlendMode::ALL {
            let mut document = sample(19, 11, 7);
            document.layers[0].blend_mode = base_mode;
            document.layers[1].blend_mode = clip_mode;
            document.layers[2].visible = false;
            document.layers[3].opacity = f32::from_bits(1);
            document.layers[4].opacity = 0.;
            let before = document.clone();
            let baked = independently_baked(&document);
            for rect in [
                PixelRect {
                    x0: 0,
                    y0: 0,
                    x1: 19,
                    y1: 11,
                },
                PixelRect {
                    x0: 3,
                    y0: 2,
                    x1: 17,
                    y1: 8,
                },
                PixelRect {
                    x0: 18,
                    y0: 10,
                    x1: 24,
                    y1: 30,
                },
            ] {
                for selection in [u64::MAX, 0b1011111, 0b1101101, 0] {
                    assert_eq!(
                        document.composite_region_layers(rect, selection),
                        baked.composite_region_layers(rect, selection)
                    );
                }
            }
            assert!(document.same_edit_state(&before));
            assert_eq!(document, before);
        }
    }
}

#[test]
fn effective_alpha_rounding_edges_and_no_mask_fast_paths_are_exact() {
    for alpha in 0..=255 {
        for coverage in 0..=255 {
            assert_eq!(
                masked_alpha(alpha, coverage),
                ((f64::from(alpha) * f64::from(coverage) / 255.).round()) as u8
            );
        }
    }
    for width in [1, 4096] {
        let mut document = sample(width, 1, 64);
        let baked = independently_baked(&document);
        assert_eq!(document.composite(), baked.composite());
        for layer in &mut document.layers {
            layer.mask = None;
        }
        let expected = document.composite();
        for coverage in [
            MaskCoverage::Uniform(255),
            MaskCoverage::Dense(Arc::new(vec![255; width as usize])),
        ] {
            for layer in &mut document.layers {
                layer.mask = Some(LayerMask {
                    enabled: true,
                    coverage: coverage.clone(),
                });
            }
            assert_eq!(document.composite(), expected);
        }
        for layer in &mut document.layers {
            layer.mask = Some(LayerMask {
                enabled: false,
                coverage: MaskCoverage::Uniform(0),
            });
        }
        assert_eq!(document.composite(), expected);
    }
}

#[test]
fn create_remove_enable_and_apply_preserve_pixels_identity_no_ops_and_lock_guards() {
    let mut document = Document::new(3, 2);
    let original = document.clone();
    assert!(!document.create_layer_mask(0, 0));
    assert!(!document.create_layer_mask(20, 0));
    assert!(document.create_layer_mask(1, 255));
    assert!(!document.create_layer_mask(1, 0));
    let masked = document.clone();
    assert!(!document.set_layer_mask_enabled(1, true));
    assert_eq!(document, masked);
    assert!(document.apply_layer_mask(1));
    assert!(document.same_edit_state(&original));
    assert!(Arc::ptr_eq(
        &document.layers[1].pixels,
        &original.layers[1].pixels
    ));
    assert!(!document.apply_layer_mask(1));
    assert!(document.create_layer_mask(1, 0));
    document.layers[1].locked = true;
    assert!(!document.remove_layer_mask(1));
    assert!(!document.apply_layer_mask(1));
    assert!(document.set_layer_mask_enabled(1, false));
    document.layers[1].locked = false;
    document.layers[1].alpha_locked = true;
    assert!(!document.apply_layer_mask(1));
    document.layers[1].alpha_locked = false;
    assert!(document.apply_layer_mask(1));
    assert!(Arc::ptr_eq(
        &document.layers[1].pixels,
        &original.layers[1].pixels
    ));
    assert!(document.create_layer_mask(1, 0));
    assert!(document.remove_layer_mask(1));
    assert!(!document.remove_layer_mask(1));
    assert!(Arc::ptr_eq(
        &document.layers[1].pixels,
        &original.layers[1].pixels
    ));
}

#[test]
fn mask_history_cow_apply_and_no_op_preserve_redo_and_raw_rgb() {
    let mut document = sample(7, 5, 3);
    let before = document.clone();
    let mut history = History::default();
    history.begin(&document, "Edit mask");
    if let MaskCoverage::Dense(values) = &mut document.layers[0].mask.as_mut().unwrap().coverage {
        Arc::make_mut(values)[0] = 255;
    }
    assert!(history.commit(&document));
    assert!(Arc::ptr_eq(
        &document.layers[0].pixels,
        &before.layers[0].pixels
    ));
    history.undo(&mut document);
    assert!(document.same_edit_state(&before));
    assert!(history.can_redo());
    history.begin(&document, "No change");
    assert!(!document.set_layer_mask_enabled(0, true));
    assert!(!history.commit(&document));
    assert!(history.can_redo());
    history.redo(&mut document);
    let rendered = document.composite();
    let original = document.clone();
    history.begin(&document, "Apply mask");
    assert!(document.apply_layer_mask(0));
    assert!(history.commit(&document));
    assert_eq!(document.composite(), rendered);
    for (current, previous) in document.layers[0]
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(original.layers[0].pixels.as_chunks::<4>().0)
    {
        assert_eq!(current[..3], previous[..3]);
    }
    history.undo(&mut document);
    assert!(document.same_edit_state(&original));
}

#[test]
fn hit_testing_and_combine_commands_respect_masks_and_do_not_apply_twice() {
    for action in [
        LayerCombine::MergeDown,
        LayerCombine::MergeSelected,
        LayerCombine::MergeVisible,
        LayerCombine::CopyVisible,
        LayerCombine::Flatten,
    ] {
        let mut document = sample(
            13,
            9,
            if action == LayerCombine::MergeDown {
                2
            } else {
                3
            },
        );
        let expected = document.composite();
        let selected = (1 << document.layers.len()) - 1;
        assert!(document.combine_layers(action, selected));
        let output = if action == LayerCombine::CopyVisible {
            &document.layers.last().unwrap().pixels
        } else {
            &document.layers[document.active].pixels
        };
        assert_eq!(output.as_slice(), expected);
        assert!(document.layers[document.active].mask.is_none());
        if action != LayerCombine::CopyVisible {
            assert_eq!(document.composite(), expected);
        }
    }
    let mut document = Document::new(1, 1);
    Arc::make_mut(&mut document.layers[1].pixels).copy_from_slice(&[10, 20, 30, 255]);
    assert!(document.create_layer_mask(1, 0));
    assert_eq!(document.layer_at(0, 0), Some(0));
    assert!(document.set_layer_mask_enabled(1, false));
    assert_eq!(document.layer_at(0, 0), Some(1));
    document.layers[0].locked = false;
    assert!(
        document
            .layer_combine_reason(LayerCombine::TransferDown, 2)
            .unwrap()
            .contains("mask")
    );
    let before = document.clone();
    assert!(!document.combine_layers(LayerCombine::TransferDown, 2));
    assert_eq!(document, before);
    assert!(document.combine_layers(LayerCombine::Clear, 2));
    assert_eq!(document.layers[1].mask, before.layers[1].mask);
}

#[test]
fn dense_mask_lengths_and_reserved_logical_memory_are_validated() {
    let mut document = Document::new(3, 2);
    document.layers[1].mask = Some(LayerMask {
        enabled: false,
        coverage: MaskCoverage::Dense(Arc::new(vec![255; 5])),
    });
    assert!(!document.validate());
    assert!(!document.apply_layer_mask(1));
    document.layers[1].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Uniform(255),
    });
    assert!(document.validate());
    assert_eq!(document.logical_pixel_bytes(), 54);
    let mut large = Document::new(4096, 4096);
    large.layers[1].pixels = large.layers[0].pixels.clone();
    large.layers.resize(7, large.layers[1].clone());
    large.next_layer_id = 8;
    for (index, layer) in large.layers.iter_mut().enumerate() {
        layer.id = crate::LayerId(index as u64 + 1);
        layer.locked = false;
    }
    for index in 0..4 {
        assert!(large.create_layer_mask(index, 255));
    }
    assert_eq!(large.logical_pixel_bytes(), 512 * 1024 * 1024);
    assert!(large.validate());
    let before = large.clone();
    assert!(!large.create_layer_mask(4, 255));
    assert_eq!(large, before);
    large.add_layer();
    assert_eq!(large, before);
    assert!(
        large
            .layer_combine_reason(LayerCombine::CopyVisible, 1)
            .unwrap()
            .contains("budget")
    );
    assert!(!large.combine_layers(LayerCombine::CopyVisible, 1));
    assert_eq!(large, before);
    let mut invalid = Document::new(1, 1);
    invalid.width = u32::MAX;
    invalid.height = u32::MAX;
    let before = invalid.clone();
    invalid.add_layer();
    assert_eq!(invalid, before);
}
