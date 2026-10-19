use super::*;
use crate::{History, LayerMask, MaskCoverage, PixelRect};

#[test]
fn subtree_move_duplicate_delete_preserve_identity_buffers_and_history() {
    let mut document = fixture();
    let folder = document
        .group_layers_in_folder(0b110, "Folder".into())
        .unwrap();
    let before = document.clone();
    let mut history = History::default();
    history.begin(&document, "Duplicate subtree");
    let copied = document.duplicate_layer_subtrees(0b1100).unwrap();
    assert_eq!(copied, 1 << 6);
    assert_eq!(document.layers.len(), 8);
    assert!(document.validate());
    let duplicate = document.layers[6].id;
    assert_ne!(duplicate, folder);
    assert_eq!(document.layers[6].name, "Folder copy");
    for (source, target) in [(1, 4), (2, 5)] {
        assert_eq!(document.layers[target].parent, Some(duplicate));
        assert_eq!(document.layers[target].name, before.layers[source].name);
        assert!(Arc::ptr_eq(
            &document.layers[target].pixels,
            &before.layers[source].pixels
        ));
        assert_ne!(document.layers[target].id, before.layers[source].id);
    }
    assert!(history.commit(&document));
    assert!(history.undo(&mut document));
    assert!(document.same_edit_state(&before));
    assert!(history.redo(&mut document));
    assert!(document.delete_layer_subtrees(copied));
    assert!(document.same_edit_state(&before));
    let raster = document.layers[2].id;
    assert!(document.move_layer_subtrees(1 << 2, 4, None));
    assert_eq!(document.layers[3].id, raster);
    assert_eq!(document.layers[3].parent, None);
    assert!(document.move_layer_subtrees(1 << 3, 2, Some(folder)));
    assert!(document.same_edit_state(&before));
    assert!(!document.move_layer_subtrees(1 << 3, 3, Some(folder)));
    assert!(!document.move_layer_subtrees(1 << 3, 4, None));
    document.layers[3].locked = true;
    assert!(!document.move_layer_subtrees(1 << 1, 4, None));
    assert!(!document.delete_layer_subtrees(1 << 3));
    assert!(document.duplicate_layer_subtrees(1 << 1).is_none());
}

#[test]
fn sibling_orders_keep_nested_branches_contiguous_and_refuse_mixed_parents() {
    let mut document = fixture();
    document
        .group_layers_in_folder(1 << 1, "Folder".into())
        .unwrap();
    assert_eq!(
        document.layer_sibling_order(1 << 2, true, false),
        Some(vec![0, 3, 1, 2, 4])
    );
    assert_eq!(
        document.layer_sibling_order(1 << 2, false, true),
        Some(vec![1, 2, 0, 3, 4])
    );
    assert!(
        document
            .layer_sibling_order((1 << 1) | (1 << 3), true, true)
            .is_none()
    );
    assert!(document.layer_tree_delete_reason(u64::MAX).is_some());
    assert!(document.layer_tree_delete_reason(0b11111).is_some());
    assert!(document.layer_reparent_reason(1 << 3, 2, None).is_some());
}

#[test]
fn folder_mask_stroke_changes_coverage_without_allocating_folder_pixels() {
    let mut document = fixture();
    document
        .group_layers_in_folder(0b110, "Folder".into())
        .unwrap();
    assert!(document.create_layer_mask(3, 255));
    let before = document.clone();
    let brush = crate::Brush {
        size: 4.,
        erase: true,
        color: [0, 0, 0],
        ..Default::default()
    };
    let mut stroke = crate::Stroke::begin_mask(
        &mut document,
        brush,
        crate::Point {
            x: 2.,
            y: 1.,
            pressure: 1.,
        },
    )
    .unwrap();
    assert!(stroke.take_damage().is_some());
    assert!(document.layers[3].pixels.is_empty());
    assert_ne!(document.layers[3].mask, before.layers[3].mask);
    for index in 0..3 {
        assert!(Arc::ptr_eq(
            &document.layers[index].pixels,
            &before.layers[index].pixels
        ));
    }
    assert_ne!(document.composite(), before.composite());
    assert!(document.validate());
}

fn fixture() -> Document {
    let mut document = Document::new(5, 3);
    document.layers[0].locked = false;
    document.add_layer();
    document.add_layer();
    for (index, layer) in document.layers.iter_mut().enumerate() {
        layer.pixels = Arc::new(
            (0..15)
                .flat_map(|pixel| {
                    [
                        (index * 63 + pixel * 17) as u8,
                        (pixel * 37 + 10) as u8,
                        (index * 81 + pixel * 23) as u8,
                        (pixel * 29 + index * 61) as u8,
                    ]
                })
                .collect(),
        );
        layer.opacity = 0.3 + index as f32 * 0.15;
    }
    document
}

fn baked_layer(document: &Document, index: usize) -> Layer {
    let mut result = document.layers[index].clone();
    result.parent = None;
    if result.kind == LayerKind::Folder {
        let mut isolated = Document::new(document.width, document.height);
        isolated.active = 0;
        isolated.next_layer_id = document.next_layer_id;
        isolated.layers = document
            .layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.parent == Some(result.id))
            .map(|(child, _)| baked_layer(document, child))
            .collect();
        result.pixels = Arc::new(if isolated.layers.is_empty() {
            vec![0; (document.width * document.height * 4) as usize]
        } else {
            isolated.composite()
        });
        result.kind = LayerKind::Raster;
    }
    result
}

fn baked(document: &Document) -> Document {
    let mut result = document.clone();
    result.layers = document
        .layers
        .iter()
        .enumerate()
        .filter(|(_, layer)| layer.parent.is_none())
        .map(|(index, _)| baked_layer(document, index))
        .collect();
    result.active = 0;
    result
}

#[test]
fn grouping_ungrouping_preserves_ids_content_order_and_named_history() {
    let mut document = fixture();
    let before = document.clone();
    let mut history = History::default();
    history.begin(&document, "Group layers");
    let folder = document
        .group_layers_in_folder(0b110, "線画".into())
        .unwrap();
    assert!(document.validate());
    assert_eq!(document.layers[3].id, folder);
    assert!(document.layers[3].pixels.is_empty());
    assert_eq!(document.layers[1].parent, Some(folder));
    assert_eq!(document.layers[2].parent, Some(folder));
    for index in 0..3 {
        assert!(Arc::ptr_eq(
            &document.layers[index].pixels,
            &before.layers[index].pixels
        ));
    }
    assert!(history.commit(&document));
    let grouped = document.clone();
    assert!(history.undo(&mut document));
    assert!(document.same_edit_state(&before));
    assert!(history.redo(&mut document));
    assert!(document.same_edit_state(&grouped));
    assert!(document.ungroup_layer_folder(3));
    assert!(document.same_edit_state(&before));
    assert_eq!(document.next_layer_id, grouped.next_layer_id);
}

#[test]
fn nested_creation_insertion_and_nonadjacent_siblings_are_real_subtrees() {
    let mut document = fixture();
    let original = document
        .layers
        .iter()
        .map(|layer| layer.id)
        .collect::<Vec<_>>();
    let folder = document
        .group_layers_in_folder(0b1010, "Folder".into())
        .unwrap();
    assert_eq!(
        document
            .layers
            .iter()
            .filter(|layer| layer.parent == Some(folder))
            .map(|layer| layer.id)
            .collect::<Vec<_>>(),
        vec![original[1], original[3]]
    );
    assert!(document.validate());
    let folder_index = document.layer_index(folder).unwrap();
    document.active = folder_index;
    document.add_layer();
    assert_eq!(document.layers[document.active].parent, Some(folder));
    assert_eq!(document.active + 1, document.layer_index(folder).unwrap());
    let nested = document.create_layer_folder("Nested".into()).unwrap();
    assert_eq!(document.layers[document.active].id, nested);
    assert_eq!(document.layers[document.active].parent, Some(folder));
    document.add_layer();
    assert_eq!(document.layers[document.active].parent, Some(nested));
    assert!(document.validate());
    assert!(document.ungroup_layer_folder(document.layer_index(nested).unwrap()));
    assert!(document.validate());
    assert_eq!(document.layers[document.active].parent, Some(folder));
    let all = (1 << document.layers.len()) - 1;
    assert!(
        document
            .group_layers_in_folder(all, "All roots".into())
            .is_some()
    );
    assert!(document.validate());
}

#[test]
fn invalid_parent_cycles_interleaving_and_folder_pixels_are_rejected() {
    let mut document = fixture();
    document
        .group_layers_in_folder(0b110, "Folder".into())
        .unwrap();
    for invalid in 0..6 {
        let mut broken = document.clone();
        match invalid {
            0 => broken.layers[1].parent = Some(LayerId(u64::MAX)),
            1 => broken.layers[3].parent = Some(broken.layers[3].id),
            2 => broken.layers[1].parent = Some(broken.layers[0].id),
            3 => broken.layers[2].parent = None,
            4 => broken.layers[3].pixels = broken.layers[0].pixels.clone(),
            _ => broken.layers[3].alpha_locked = true,
        }
        assert!(!broken.validate(), "invalid={invalid}");
    }
    let before = document.clone();
    assert!(
        document
            .group_layers_in_folder(0b1001, "Mixed parents".into())
            .is_some()
    );
    document = before;
    let unchanged = document.clone();
    assert!(
        document
            .group_layers_in_folder(0b101, "Mixed parents".into())
            .is_none()
    );
    assert!(document.same_edit_state(&unchanged));
}

#[test]
fn isolated_nested_folders_match_independently_baked_flat_composition_and_regions() {
    let mut document = fixture();
    document.layers[2].clipped = true;
    let inner = document
        .group_layers_in_folder(0b110, "Inner".into())
        .unwrap();
    let outer = document
        .group_layers_in_folder(0b11000, "Outer".into())
        .unwrap();
    let inner_index = document.layer_index(inner).unwrap();
    document.layers[inner_index].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Dense(Arc::new((0..15).map(|index| (index * 17) as u8).collect())),
    });
    let inner_index = document.layer_index(inner).unwrap();
    let outer_index = document.layer_index(outer).unwrap();
    document.layers[inner_index].opacity = 0.63;
    document.layers[outer_index].opacity = 0.47;
    document.layers[outer_index].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Uniform(193),
    });
    for inner_mode in BlendMode::ALL {
        document.layers[inner_index].blend_mode = inner_mode;
        for outer_mode in BlendMode::ALL {
            document.layers[outer_index].blend_mode = outer_mode;
            let reference = baked(&document);
            assert_eq!(
                document.composite(),
                reference.composite(),
                "{inner_mode:?}/{outer_mode:?}"
            );
            let region = PixelRect {
                x0: 1,
                y0: 1,
                x1: 4,
                y1: 3,
            };
            assert_eq!(
                document.composite_region(region),
                reference.composite_region(region)
            );
        }
    }
}

#[test]
fn sibling_clipping_cannot_escape_a_folder_and_folder_bases_are_supported() {
    let mut document = fixture();
    let folder = document
        .group_layers_in_folder(0b110, "Folder".into())
        .unwrap();
    document.layers[1].clipped = true;
    assert_eq!(document.clipping_base(1), None);
    assert_eq!(document.clipping_base(2), Some(2));
    document.layers[1].clipped = false;
    document.layers[4].clipped = true;
    document.layers[4].blend_mode = BlendMode::Multiply;
    assert_eq!(document.clipping_base(4), document.layer_index(folder));
    assert_eq!(document.composite(), baked(&document).composite());
    document.layers[3].visible = false;
    assert_eq!(document.composite(), baked(&document).composite());
}

#[test]
fn ancestor_visibility_locks_and_hit_testing_apply_to_children_not_folder_pixels() {
    let mut document = Document::new(1, 1);
    document.layers[1].pixels = Arc::new(vec![20, 40, 60, 255]);
    let folder = document.group_layers_in_folder(2, "Folder".into()).unwrap();
    let parent = document.layer_index(folder).unwrap();
    assert_eq!(document.layer_at(0, 0), Some(1));
    assert!(document.layer_ancestors_allow_edit(1));
    document.layers[parent].locked = true;
    assert!(!document.layer_ancestors_allow_edit(1));
    assert_eq!(document.layer_at(0, 0), Some(1));
    document.layers[parent].locked = false;
    document.layers[parent].visible = false;
    assert!(!document.layer_ancestors_allow_edit(1));
    assert!(document.layer_ancestors_unlocked(1));
    assert_eq!(document.layer_at(0, 0), Some(0));
    document.layers[parent].visible = true;
    document.layers[parent].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Uniform(0),
    });
    assert_eq!(document.layer_at(0, 0), Some(0));
}

#[test]
fn sixty_four_deep_nodes_use_no_folder_pixel_storage_and_keep_atomic_limits() {
    let mut document = Document::new(1, 1);
    document.layers.remove(0);
    document.active = 0;
    document.layers[0].pixels = Arc::new(vec![30, 60, 90, 255]);
    for _ in 0..63 {
        document
            .group_layers_in_folder(1 << document.active, "Folder".into())
            .unwrap();
    }
    assert!(document.validate());
    assert_eq!(document.logical_pixel_bytes(), 4);
    assert_eq!(document.layer_subtree_mask(63), Some(u64::MAX));
    assert_eq!(document.composite(), vec![30, 60, 90, 255]);
    let before = document.clone();
    assert!(document.create_layer_folder("Overflow".into()).is_none());
    assert!(document.same_edit_state(&before));
    assert_eq!(document.next_layer_id, before.next_layer_id);
}

#[test]
fn folder_clipping_base_and_source_match_flat_baking_for_every_blend_pair() {
    for folder_is_clip in [false, true] {
        let mut document = fixture();
        document
            .group_layers_in_folder(0b110, "Folder".into())
            .unwrap();
        let (base, clip) = if folder_is_clip { (0, 3) } else { (3, 4) };
        document.layers[clip].clipped = true;
        document.layers[base].mask = Some(LayerMask {
            enabled: true,
            coverage: MaskCoverage::Uniform(113),
        });
        document.layers[clip].mask = Some(LayerMask {
            enabled: true,
            coverage: MaskCoverage::Dense(Arc::new(
                (0..15).map(|pixel| (pixel * 19) as u8).collect(),
            )),
        });
        for base_mode in BlendMode::ALL {
            document.layers[base].blend_mode = base_mode;
            for clip_mode in BlendMode::ALL {
                document.layers[clip].blend_mode = clip_mode;
                assert_eq!(
                    document.composite(),
                    baked(&document).composite(),
                    "folder_clip={folder_is_clip}/{base_mode:?}/{clip_mode:?}"
                );
            }
        }
    }
}

#[test]
fn folder_and_locked_ancestor_refuse_all_pixel_writers_without_copying_buffers() {
    use crate::{Brush, BrushOperation, Point, RasterConversion, Stroke};
    let mut document = Document::new(8, 8);
    document.group_layers_in_folder(2, "Folder".into()).unwrap();
    let point = Point {
        x: 4.,
        y: 4.,
        pressure: 1.,
    };
    for operation in [
        BrushOperation::Paint,
        BrushOperation::Erase,
        BrushOperation::Blend,
        BrushOperation::Blur,
        BrushOperation::Smudge,
    ] {
        let before = document.clone();
        let brush = Brush {
            operation,
            ..Default::default()
        };
        let mut stroke = Stroke::begin(&mut document, brush, point);
        stroke.extend(&mut document, Point { x: 5., ..point });
        assert!(document.same_edit_state(&before));
        assert_eq!(document.revision, before.revision);
    }
    assert!(!document.convert_layer_pixels(2, RasterConversion::DrawingColor([10, 20, 30])));
    document.layers[2].locked = true;
    document.active = 1;
    let before = document.clone();
    Stroke::begin(&mut document, Brush::default(), point);
    assert!(document.same_edit_state(&before));
    assert!(!document.convert_layer_pixels(1, RasterConversion::BrightnessToOpacity));
    assert!(!document.create_layer_mask(1, 128));
    assert!(
        document
            .create_layer_folder("Locked parent".into())
            .is_none()
    );
    assert!(
        document
            .group_layers_in_folder(2, "Locked parent".into())
            .is_none()
    );
    assert!(!document.ungroup_layer_folder(2));
    assert!(document.same_edit_state(&before));
}

#[test]
#[ignore = "release-only folder and flat composition workload"]
fn folder_composition_timing_workload() {
    use std::{hint::black_box, time::Instant};
    if cfg!(debug_assertions) {
        panic!("Run with --release");
    }
    for side in [256, 512] {
        let mut document = Document::new(side, side);
        document.layers[0].locked = false;
        for _ in 0..6 {
            document.add_layer();
        }
        for (index, layer) in document.layers.iter_mut().enumerate() {
            layer.pixels = Arc::new(
                (0..side * side)
                    .flat_map(|pixel| {
                        [
                            (pixel * 17) as u8,
                            (pixel * 31 + index as u32 * 7) as u8,
                            (pixel * 11) as u8,
                            (pixel * 23 + index as u32 * 47) as u8,
                        ]
                    })
                    .collect(),
            );
        }
        document
            .group_layers_in_folder(0b11110000, "Inner".into())
            .unwrap();
        document
            .group_layers_in_folder((1 << 8) | 0b1111, "Outer".into())
            .unwrap();
        let reference = baked(&document);
        let expected = reference.composite();
        assert_eq!(document.composite(), expected);
        for (label, drawing) in [("flat_baked", &reference), ("folders", &document)] {
            let mut elapsed = Vec::new();
            for _ in 0..25 {
                let started = Instant::now();
                let output = black_box(drawing).composite();
                elapsed.push(started.elapsed().as_secs_f64() * 1000.);
                assert_eq!(output, expected);
            }
            elapsed.sort_by(f64::total_cmp);
            eprintln!(
                "{side} {label}: p50_ms={:.4} p95_ms={:.4} row_scratch_bytes={}",
                elapsed[12],
                elapsed[23],
                drawing
                    .layers
                    .iter()
                    .filter(|layer| layer.kind == LayerKind::Folder)
                    .count()
                    * side as usize
                    * 4
            );
        }
    }
}
