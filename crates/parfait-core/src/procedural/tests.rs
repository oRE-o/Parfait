use super::*;
use crate::{
    Brush, BrushOperation, History, PixelRect, Point, SelectionMode, Stroke,
};

fn baked(document: &Document) -> Document {
    let mut output = document.clone();
    for layer in &mut output.layers {
        if let Some([red, green, blue]) = layer.kind.solid_color() {
            layer.kind = LayerKind::Raster;
            layer.pixels = Arc::new(
                [red, green, blue, 255]
                    .repeat((document.width * document.height) as usize),
            );
        }
    }
    assert!(output.validate());
    output
}

fn compare(document: &Document) {
    assert!(document.validate());
    let reference = baked(document);
    assert_eq!(document.composite(), reference.composite());
    assert_eq!(
        document.composite_for_export(),
        reference.composite_for_export()
    );
    let region = PixelRect {
        x0: 1,
        y0: 1,
        x1: document.width - 1,
        y1: document.height - 1,
    };
    assert_eq!(
        document.composite_region(region),
        reference.composite_region(region)
    );
    for row in 0..document.height {
        for column in 0..document.width {
            assert_eq!(
                document.layer_at(column, row),
                reference.layer_at(column, row)
            );
        }
    }
}

#[test]
fn procedural_rows_match_materialized_pixels_for_all_blends_clips_and_partial_regions()
 {
    for base in BlendMode::ALL {
        for clip in BlendMode::ALL {
            let mut document = Document::new_with_paper(8, 5);
            document.layers[1].pixels = Arc::new(
                (0..40)
                    .flat_map(|index| {
                        [index as u8 * 3, 90, 170, (index * 19) as u8]
                    })
                    .collect(),
            );
            document
                .create_solid_layer([70, 150, 210], false, "Fill".into())
                .unwrap();
            document.layers[2].blend_mode = base;
            document.layers[2].opacity = 0.63;
            document.layers[2].mask.as_mut().unwrap().coverage =
                MaskCoverage::Dense(Arc::new(
                    (0..40).map(|index| (index * 37) as u8).collect(),
                ));
            document.add_layer();
            document.layers[3].pixels = Arc::new(
                (0..40)
                    .flat_map(|index| [180, 50, 90, (index * 13) as u8])
                    .collect(),
            );
            document.layers[3].blend_mode = clip;
            document.layers[3].clipped = true;
            document.layers[3].opacity = 0.7;
            compare(&document);
        }
    }
}

#[test]
fn fill_composes_inside_nested_through_folders_with_masks_and_draft_export() {
    let mut document = Document::new_with_paper(8, 5);
    document.create_solid_layer([70, 150, 210], false, "Fill".into()).unwrap();
    document.layers[2].opacity = 0.4;
    document.group_layers_in_folder(6, "Inner".into()).unwrap();
    document.layers[3].blend_mode = BlendMode::Through;
    document.layers[3].opacity = 0.65;
    document.create_layer_mask(3, 193);
    document.group_layers_in_folder(8, "Outer".into()).unwrap();
    for outer in [BlendMode::Normal, BlendMode::Multiply, BlendMode::Through] {
        document.layers[4].blend_mode = outer;
        compare(&document);
        document.layers[2].draft = true;
        compare(&document);
        document.layers[2].draft = false;
    }
    document.layers[3].visible = false;
    compare(&document);
}

#[test]
fn paper_is_unique_root_and_cannot_be_grouped_moved_masked_or_duplicated() {
    let mut document = Document::new_with_paper(8, 5);
    assert!(document.validate());
    assert_eq!(document.logical_pixel_bytes(), 8 * 5 * 4);
    assert!(document.layers[0].pixels.is_empty());
    let before = document.clone();
    assert!(
        document.create_solid_layer([10; 3], true, "Another".into()).is_none()
    );
    assert!(document.group_layers_in_folder(1, "Invalid".into()).is_none());
    assert!(!document.move_layer_subtrees(1, 2, None));
    assert!(!document.move_layer_subtrees(2, 0, None));
    assert!(document.duplicate_layer_subtrees(1).is_none());
    assert!(!document.create_layer_mask(0, 255));
    assert!(document.layer_sibling_order(1, true, false).is_none());
    assert!(document.layer_sibling_order(2, false, true).is_none());
    assert!(document.same_edit_state(&before));
    assert_eq!(document.revision, before.revision);
    assert!(document.delete_layer_subtrees(1));
    let id = document
        .create_solid_layer([10, 40, 90], true, "Paper".into())
        .unwrap();
    assert_eq!(document.layers[0].id, id);
    assert!(document.validate());
    assert!(document.layers[0].pixels.is_empty());
    document.active = 1;
    document.create_solid_layer([30; 3], false, "Fill".into()).unwrap();
    assert_eq!(
        document.layer_sibling_order(4, false, true).unwrap(),
        [0, 2, 1]
    );
    let mut damaged = document.clone();
    damaged.layers[0].opacity = 0.5;
    assert!(!damaged.validate());
    damaged = document.clone();
    damaged.layers.swap(0, 1);
    assert!(!damaged.validate());
    document.layers[0].locked = true;
    assert!(!document.set_solid_layer_color(0, [50; 3]));
    assert!(!document.rasterize_solid_layer(0));
    assert!(!document.delete_layer_subtrees(1));
}

#[test]
fn fill_shares_selection_and_preserves_parameters_through_history_rasterization_and_mask_writes()
 {
    let mut document = Document::new_with_paper(8, 5);
    document.set_selection(Some(MaskCoverage::Dense(Arc::new(
        (0..40).map(|index| (index * 7) as u8).collect(),
    ))));
    let before = document.clone();
    let mut history = History::default();
    history.begin(&document, "Fill");
    document.create_solid_layer([30, 80, 160], false, "Fill".into()).unwrap();
    history.commit(&document);
    assert!(
        document.layers[2]
            .mask
            .as_ref()
            .unwrap()
            .coverage
            .same_edit_state(document.selection.as_ref().unwrap())
    );
    let created = document.clone();
    history.undo(&mut document);
    assert!(document.same_edit_state(&before));
    history.redo(&mut document);
    assert!(document.same_edit_state(&created));
    let revision = document.revision;
    assert!(!document.set_solid_layer_color(2, [30, 80, 160]));
    assert_eq!(document.revision, revision);
    assert!(document.set_solid_layer_color(2, [90, 60, 20]));
    let point = Point { x: 4., y: 2., pressure: 1. };
    let before_paint = document.clone();
    Stroke::begin(&mut document, Brush::default(), point);
    assert!(document.same_edit_state(&before_paint));
    assert_eq!(document.revision, before_paint.revision);
    let mask_before = document.layers[2].mask.clone();
    let brush = Brush {
        operation: BrushOperation::Erase,
        size: 100.,
        pressure: false,
        hardness: 1.,
        ..Default::default()
    };
    Stroke::begin_mask(&mut document, brush, point).unwrap();
    assert_ne!(document.layers[2].mask, mask_before);
    assert!(document.layers[2].pixels.is_empty());
    document.deselect_pixels();
    let pixels = document.composite();
    let before = document.clone();
    history.begin(&document, "Rasterize");
    assert!(document.rasterize_solid_layer(2));
    history.commit(&document);
    assert_eq!(document.composite(), pixels);
    assert_eq!(document.layers[2].kind, LayerKind::Raster);
    assert!(
        document.layers[2]
            .mask
            .as_ref()
            .unwrap()
            .coverage
            .same_edit_state(&before.layers[2].mask.as_ref().unwrap().coverage)
    );
    history.undo(&mut document);
    assert!(document.same_edit_state(&before));
    assert!(document.selection_from_layers(4, SelectionMode::Replace));
    assert!(document.validate());
}

#[test]
fn solid_creation_respects_locked_parents_and_sixty_four_node_limit_without_canvas_copies()
 {
    let mut document = Document::new_with_paper(4096, 1);
    let paper_pixels = document.layers[0].pixels.clone();
    document.create_layer_folder("Folder".into()).unwrap();
    document.layers[2].locked = true;
    assert!(
        document.create_solid_layer([20; 3], false, "Fill".into()).is_none()
    );
    document.layers[2].locked = false;
    let parent = document.layers[2].id;
    document.create_solid_layer([20; 3], false, "Fill".into()).unwrap();
    assert_eq!(document.layers[2].parent, Some(parent));
    while document.layers.len() < 64 {
        document.create_solid_layer([20; 3], false, "Fill".into()).unwrap();
    }
    let before = document.clone();
    assert!(
        document.create_solid_layer([20; 3], false, "Extra".into()).is_none()
    );
    assert!(document.same_edit_state(&before));
    assert!(Arc::ptr_eq(&paper_pixels, &document.layers[0].pixels));
    assert_eq!(
        document.layers.iter().map(|layer| layer.pixels.len()).sum::<usize>(),
        4096 * 4
    );
    compare(&document);
}
