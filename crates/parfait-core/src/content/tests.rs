use super::*;
use crate::{Align, BufferCensus, CanvasEdit, History, PixelEdit};
fn vector(width: f32) -> LayerContent {
    LayerContent::Vector(VectorContent {
        matrix: Affine::translation(8., 8.),
        paths: vec![VectorPath {
            points: Arc::new(vec![[0., 0.], [20., 10.]]),
            color: [10, 50, 100, 200],
            width,
            closed: false,
            fill: false,
        }],
    })
}
fn text(s: &str) -> LayerContent {
    LayerContent::Text(TextContent{text:s.into(),font:Arc::new(include_bytes!("../../../../apps/parfait/assets/pretendard/Pretendard-Regular.otf").to_vec()),size:24.,color:[20,30,40,255],matrix:Affine::translation(5.,5.)})
}
#[test]
fn text_layout_ink_and_vector_centerline_are_distinct_real_bounds() {
    let a = ContentNode::new(LayerId(3), text("그림 ")).unwrap();
    let layout = a.bounds(false, false).unwrap();
    let ink = a.bounds(true, false).unwrap();
    assert!(layout.max[0] > ink.max[0]);
    assert!(ink.min[1] > layout.min[1]);
    assert!(
        a.render(100, 60).unwrap().as_chunks::<4>().0.iter().any(|p| p[3] > 0)
    );
    let v = ContentNode::new(LayerId(4), vector(10.)).unwrap();
    assert_eq!(
        v.bounds(false, true).unwrap(),
        ContentBox { min: [8., 8.], max: [28., 18.] }
    );
    assert_eq!(
        v.bounds(false, false).unwrap(),
        ContentBox { min: [3., 3.], max: [33., 23.] }
    );
    let svg = a.svg(100, 60).unwrap();
    assert!(svg.contains("<path"));
    assert!(!svg.contains("<text"));
    assert!(!svg.contains("href"));
}
#[test]
fn alignment_options_change_plan_and_translation_preserves_editable_source() {
    let mut doc = Document::new(80, 60);
    doc.create_content_layer(vector(10.), "A".into()).unwrap();
    doc.create_content_layer(vector(2.), "B".into()).unwrap();
    let selected = (1 << 2) | (1 << 3);
    let outline = doc
        .plan_content_alignment(
            &doc.content_alignment_bounds(selected, false, false).unwrap(),
            Align::Top,
            None,
        )
        .unwrap();
    let center = doc
        .plan_content_alignment(
            &doc.content_alignment_bounds(selected, false, true).unwrap(),
            Align::Top,
            None,
        )
        .unwrap();
    assert_ne!(outline, center);
    let before = doc.clone();
    let mut history = History::default();
    history.begin(&doc, "align");
    assert!(doc.translate_layer_pixels(&outline));
    assert!(doc.validate());
    history.commit(&doc);
    for (index, l) in doc.layers.iter().enumerate() {
        if let Some(n) = doc.native_content(index) {
            assert_eq!(l.pixels, n.render(doc.width, doc.height).unwrap());
        }
    }
    assert!(history.undo(&mut doc));
    assert_eq!(doc.art_digest_v1(), before.art_digest_v1());
    assert!(history.redo(&mut doc));
    assert!(doc.validate());
}
#[test]
fn native_history_duplication_mask_and_rasterization_preserve_pixels() {
    let mut doc = Document::new(70, 50);
    let id = doc.create_content_layer(text("가"), "Text".into()).unwrap();
    let before = doc.clone();
    let mut census = BufferCensus::default();
    census.add_document(&doc);
    let bytes = census.bytes();
    census.add_document(&doc.clone());
    assert_eq!(bytes, census.bytes());
    let copied = doc.duplicate_layer_subtrees(1 << doc.active).unwrap();
    assert_eq!(doc.contents.len(), 2);
    let copied_index = copied.trailing_zeros() as usize;
    assert!(Arc::ptr_eq(
        doc.native_content(copied_index).unwrap().body(),
        before.native_content(before.active).unwrap().body()
    ));
    let index = doc.layer_index(id).unwrap();
    assert!(!doc.edit_layer_pixels(index, PixelEdit::Fill([0; 4])));
    assert!(doc.create_layer_mask(index, 128));
    assert!(!doc.apply_layer_mask(index));
    let pixels = doc.layers[index].pixels.clone();
    assert!(doc.rasterize_content(index));
    assert!(Arc::ptr_eq(&pixels, &doc.layers[index].pixels));
    assert!(doc.apply_layer_mask(index));
    assert!(doc.validate());
    assert!(doc.delete_layer_subtrees(copied));
    assert!(doc.contents.is_empty());
}
#[test]
fn canvas_geometry_updates_models_and_cache_atomically() {
    let mut doc = Document::new(80, 50);
    doc.create_content_layer(vector(5.), "V".into()).unwrap();
    let before = doc.clone();
    for _ in 0..4 {
        assert!(doc.edit_canvas(CanvasEdit::RotateRight));
        assert!(doc.validate());
    }
    assert_eq!(doc.contents, before.contents);
    assert_eq!(doc.composite(), before.composite());
    assert!(doc.edit_canvas(CanvasEdit::Resize { width: 160, height: 100 }));
    let n = doc.native_content(doc.active).unwrap();
    assert_eq!(n.body().matrix().0, [2., 0., 0., 2., 16., 16.]);
    assert_eq!(doc.layers[doc.active].pixels, n.render(160, 100).unwrap());
}
#[test]
fn invalid_bounded_content_never_mutates_document() {
    let mut doc = Document::new(32, 32);
    let before = doc.clone();
    let mut bad = vector(2.);
    if let LayerContent::Vector(v) = &mut bad {
        v.matrix.0[0] = f32::NAN;
    }
    assert!(doc.create_content_layer(bad, "bad".into()).is_err());
    assert!(doc.same_edit_state(&before));
    assert!(
        doc.create_content_layer(text(&"a".repeat(129)), "bad".into()).is_err()
    );
    assert!(doc.same_edit_state(&before));
    assert!(doc.create_content_layer(vector(2.), "V".into()).is_ok());
    let before = doc.clone();
    let plan = [(doc.layers[doc.active].id, [2_000_000, 0])];
    assert!(!doc.translate_layer_pixels(&plan));
    assert!(doc.same_edit_state(&before));
}
