use super::*;
use crate::{Brush, BrushOperation, BrushTip, History, OpacityMode, Point, Stroke};

#[test]
fn explicit_clear_convert_and_mask_restore_obey_fractional_selection_without_changing_its_buffer() {
    let mut document = Document::new(4, 1);
    document.layers[1].pixels = Arc::new([80, 120, 160, 255].repeat(4));
    document.set_selection(Some(MaskCoverage::Dense(Arc::new(vec![0, 64, 128, 255]))));
    let before = document.clone();
    assert!(document.combine_layers(crate::LayerCombine::Clear, 2));
    assert_eq!(
        (0..4)
            .map(|index| document.layers[1].pixels[index * 4 + 3])
            .collect::<Vec<_>>(),
        [255, 191, 127, 0]
    );
    assert_eq!(
        &document.layers[1].pixels[..4],
        &before.layers[1].pixels[..4]
    );
    document = before.clone();
    assert!(document.convert_layer_pixels(1, crate::RasterConversion::DrawingColor([0; 3])));
    for index in 0..4 {
        let fraction = [0, 64, 128, 255][index] as u32;
        for (channel, initial) in [80, 120, 160].into_iter().enumerate() {
            assert_eq!(
                document.layers[1].pixels[index * 4 + channel],
                ((initial * (255 - fraction) + 127) / 255) as u8
            );
        }
        assert_eq!(document.layers[1].pixels[index * 4 + 3], 255);
    }
    document.create_layer_mask(1, 0);
    assert!(document.clear_layer_mask(1));
    assert_eq!(
        document.layers[1].mask.as_ref().unwrap().coverage,
        document.selection.clone().unwrap()
    );
    assert!(
        document
            .selection
            .as_ref()
            .unwrap()
            .same_edit_state(before.selection.as_ref().unwrap())
    );
    document.set_selection(Some(MaskCoverage::Uniform(0)));
    let empty = document.clone();
    assert!(!document.combine_layers(crate::LayerCombine::Clear, 2));
    assert!(!document.convert_layer_pixels(1, crate::RasterConversion::BrightnessToOpacity));
    assert!(!document.clear_layer_mask(1));
    assert!(document.same_edit_state(&empty));
    assert_eq!(document.revision, empty.revision);
}

#[test]
fn fractional_noop_conversion_preserves_invisible_rgb_and_original_arc() {
    let mut document = Document::new(4, 1);
    document.layers[1].pixels = Arc::new([80, 120, 160, 0].repeat(4));
    document.set_selection(Some(MaskCoverage::Uniform(127)));
    let before = document.clone();
    for conversion in [
        crate::RasterConversion::DrawingColor([0; 3]),
        crate::RasterConversion::BrightnessToOpacity,
    ] {
        assert!(!document.convert_layer_pixels(1, conversion));
        assert!(document.same_edit_state(&before));
        assert_eq!(document.revision, before.revision);
    }
}

#[test]
fn coverage_algebra_matches_independent_integer_oracles_for_every_byte_pair() {
    for previous in 0..=255u32 {
        for source in 0..=255u32 {
            for (mode, expected) in [
                (SelectionMode::Replace, source),
                (
                    SelectionMode::Add,
                    previous + ((255 - previous) * source + 127) / 255,
                ),
                (
                    SelectionMode::Subtract,
                    (previous * (255 - source) + 127) / 255,
                ),
                (SelectionMode::Intersect, (previous * source + 127) / 255),
            ] {
                assert_eq!(mode.combine(previous as u8, source as u8), expected as u8);
            }
        }
    }
}

fn source() -> Document {
    let mut document = Document::new(4, 1);
    document.layers[0].pixels = Arc::new(vec![0; 16]);
    document.layers[1].pixels = Arc::new(vec![
        40, 90, 180, 0, 40, 90, 180, 64, 40, 90, 180, 128, 40, 90, 180, 255,
    ]);
    document
}

#[test]
fn layer_geometry_preserves_soft_coverage_and_readonly_metadata_without_copying_pixels() {
    let mut document = source();
    document.layers[1].opacity = 0.25;
    document.layers[1].visible = false;
    document.layers[1].locked = true;
    let before = document.clone();
    assert!(document.selection_from_layers(2, SelectionMode::Replace));
    assert_eq!(
        (0..4)
            .map(|index| document.selection_value(index))
            .collect::<Vec<_>>(),
        [0, 64, 128, 255]
    );
    assert_eq!(document.layers, before.layers);
    assert!(Arc::ptr_eq(
        &document.layers[1].pixels,
        &before.layers[1].pixels
    ));
    let selected = document.clone();
    assert!(!document.selection_from_layers(2, SelectionMode::Replace));
    assert!(document.same_edit_state(&selected));
    assert!(document.selection_from_layers(2, SelectionMode::Add));
    assert_eq!(document.selection_value(2), 192);
    assert!(document.selection_from_layers(2, SelectionMode::Intersect));
    assert_eq!(document.selection_value(2), 96);
    assert!(document.selection_from_layers(2, SelectionMode::Subtract));
    assert_eq!(document.selection_value(2), 48);
    document.layers[1].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Uniform(128),
    });
    assert!(document.selection_from_layers(2, SelectionMode::Replace));
    assert_eq!(document.selection_value(3), 128);
}

#[test]
fn nested_folder_selection_normalizes_roots_and_uses_masks_once() {
    let mut document = source();
    document.group_layers_in_folder(2, "Inner".into()).unwrap();
    document.create_layer_mask(2, 128);
    document.layers[2].opacity = 0.;
    document.group_layers_in_folder(4, "Outer".into()).unwrap();
    assert!(document.selection_from_layers(0b1110, SelectionMode::Replace));
    assert_eq!(
        (0..4)
            .map(|index| document.selection_value(index))
            .collect::<Vec<_>>(),
        [0, 32, 64, 128]
    );
    document.layers[2].visible = false;
    assert!(document.selection_from_layers(8, SelectionMode::Replace));
    assert_eq!(document.selection, None);
}

#[test]
fn empty_none_invalid_and_noop_selections_keep_history_and_shared_data_predictable() {
    let mut document = source();
    let mut history = History::default();
    assert!(!document.selection_from_layers(2, SelectionMode::Add));
    for selected in [0, 1 << 63] {
        assert!(!document.selection_from_layers(selected, SelectionMode::Replace));
    }
    assert!(!document.set_selection(Some(MaskCoverage::Dense(Arc::new(vec![0; 3])))));
    history.begin(&document, "Selection");
    assert!(document.selection_from_layers(2, SelectionMode::Replace));
    assert!(history.commit(&document));
    let selected = document.clone();
    assert!(history.undo(&mut document));
    assert!(document.selection.is_none());
    history.begin(&document, "No change");
    assert!(!document.deselect_pixels());
    assert!(!history.commit(&document));
    assert!(history.can_redo());
    assert!(history.redo(&mut document));
    assert!(document.same_edit_state(&selected));
    assert!(document.invert_pixel_selection());
    assert_eq!(document.selection_value(0), 255);
    assert!(document.select_all_pixels());
    assert!(document.invert_pixel_selection());
    assert_eq!(document.selection, None);
    assert!(!document.deselect_pixels());
    assert!(document.set_selection(Some(MaskCoverage::Uniform(0))));
    assert!(document.deselect_pixels());
    assert_eq!(document.selection_value(0), 255);
}

#[test]
fn selection_masks_share_coverage_obey_locks_and_leave_original_pixels_unchanged() {
    let mut document = source();
    assert!(document.create_mask_from_selection(1, true));
    assert_eq!(
        document.layers[1].mask.as_ref().unwrap().coverage,
        MaskCoverage::Uniform(255)
    );
    assert!(document.remove_layer_mask(1));
    document.selection_from_layers(2, SelectionMode::Replace);
    let before = document.clone();
    for outside in [true, false] {
        document = before.clone();
        assert!(document.create_mask_from_selection(1, outside));
        for index in 0..4 {
            assert_eq!(
                document.layers[1]
                    .mask
                    .as_ref()
                    .unwrap()
                    .coverage
                    .value(index),
                if outside {
                    before.selection_value(index)
                } else {
                    255 - before.selection_value(index)
                }
            );
        }
        if outside {
            let Some(MaskCoverage::Dense(selection)) = &document.selection else {
                panic!()
            };
            let MaskCoverage::Dense(mask) = &document.layers[1].mask.as_ref().unwrap().coverage
            else {
                panic!()
            };
            assert!(Arc::ptr_eq(selection, mask));
        }
        assert!(Arc::ptr_eq(
            &document.layers[1].pixels,
            &before.layers[1].pixels
        ));
        assert!(!document.create_mask_from_selection(1, outside));
    }
    document = before.clone();
    document.layers[1].locked = true;
    let locked = document.clone();
    assert!(!document.create_mask_from_selection(1, true));
    assert_eq!(document, locked);
}

#[test]
fn every_brush_operation_and_tip_respects_holes_and_region_composition() {
    for operation in [
        BrushOperation::Paint,
        BrushOperation::Erase,
        BrushOperation::Blend,
        BrushOperation::Blur,
        BrushOperation::Smudge,
    ] {
        for tip in [
            BrushTip::Round,
            BrushTip::Soft,
            BrushTip::Pencil,
            BrushTip::Flat,
            BrushTip::Image {
                width: 2,
                height: 1,
                mask: Arc::new(vec![127, 255]),
            },
        ] {
            for opacity_mode in [OpacityMode::PerDab, OpacityMode::PerStroke] {
                let mut document = Document::new(32, 32);
                document.layers[1].pixels = Arc::new(
                    (0..1024)
                        .flat_map(|index| {
                            [
                                (index * 37) as u8,
                                (index * 11) as u8,
                                (index * 19) as u8,
                                128,
                            ]
                        })
                        .collect(),
                );
                document.set_selection(Some(MaskCoverage::Dense(Arc::new(
                    (0..1024)
                        .map(|index| if index % 3 == 0 { 0 } else { 128 })
                        .collect(),
                ))));
                let before = document.clone();
                let brush = Brush {
                    operation,
                    tip: tip.clone(),
                    opacity_mode,
                    size: 16.,
                    pressure: false,
                    hardness: 1.,
                    ..Default::default()
                };
                let mut stroke = Stroke::begin(
                    &mut document,
                    brush,
                    Point {
                        x: 12.,
                        y: 16.,
                        pressure: 1.,
                    },
                );
                stroke.extend(
                    &mut document,
                    Point {
                        x: 21.,
                        y: 16.,
                        pressure: 1.,
                    },
                );
                for index in (0..1024).step_by(3) {
                    assert_eq!(
                        &document.layers[1].pixels[index * 4..index * 4 + 4],
                        &before.layers[1].pixels[index * 4..index * 4 + 4],
                        "{operation:?}"
                    );
                }
                if let Some(rect) = stroke.take_damage() {
                    let region = document.composite_region(rect);
                    let full = document.composite();
                    let stride = (rect.x1 - rect.x0) as usize * 4;
                    for row in 0..(rect.y1 - rect.y0) as usize {
                        let start = ((rect.y0 as usize + row) * 32 + rect.x0 as usize) * 4;
                        assert_eq!(
                            &region[row * stride..(row + 1) * stride],
                            &full[start..start + stride]
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn empty_selection_dabs_do_not_copy_or_change_pixels_masks_revision_or_history() {
    let mut document = source();
    document.set_selection(Some(MaskCoverage::Uniform(0)));
    document.create_layer_mask(1, 127);
    for opacity_mode in [OpacityMode::PerDab, OpacityMode::PerStroke] {
        let before = document.clone();
        let brush = Brush {
            opacity_mode,
            pressure: false,
            size: 32.,
            ..Default::default()
        };
        let point = Point {
            x: 2.,
            y: 0.5,
            pressure: 1.,
        };
        let mut stroke = Stroke::begin(&mut document, brush.clone(), point);
        stroke.extend(&mut document, point);
        assert!(stroke.take_damage().is_none());
        let mut stroke = Stroke::begin_mask(&mut document, brush, point).unwrap();
        stroke.extend(&mut document, point);
        assert!(stroke.take_damage().is_none());
        assert!(document.same_edit_state(&before));
        assert_eq!(document.revision, before.revision);
    }
}

#[test]
fn capped_strokes_apply_fractional_selection_once_to_the_accumulation_limit() {
    for mask in [false, true] {
        let mut document = Document::new(8, 8);
        if mask {
            document.create_layer_mask(1, 0);
        }
        document.set_selection(Some(MaskCoverage::Uniform(128)));
        let brush = Brush {
            size: 32.,
            hardness: 1.,
            pressure: false,
            opacity_mode: OpacityMode::PerStroke,
            ..Default::default()
        };
        let point = Point {
            x: 4.,
            y: 4.,
            pressure: 1.,
        };
        let mut stroke = if mask {
            Stroke::begin_mask(&mut document, brush, point).unwrap()
        } else {
            Stroke::begin(&mut document, brush, point)
        };
        for _ in 0..50 {
            stroke.extend(
                &mut document,
                Point {
                    x: 4.5,
                    y: 4.,
                    pressure: 1.,
                },
            );
            stroke.extend(&mut document, point);
        }
        if mask {
            assert_eq!(
                document.layers[1].mask.as_ref().unwrap().coverage.value(36),
                128
            );
        } else {
            assert_eq!(document.layers[1].pixels[36 * 4 + 3], 128);
        }
    }
}
