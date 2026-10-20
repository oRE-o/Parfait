use super::*;
use crate::{LayerMask, MaskCoverage};
use std::sync::Arc;

#[test]
fn multi_document_digest_has_an_independently_encoded_fixed_vector() {
    let document = Document::new(1, 1);
    let digest = document.art_digest_v1().unwrap();
    let hex: String =
        digest.0.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(
        hex,
        "cb58d5ab406860a74314555583ef93c13eef3c5202dc78fc872e9f50d83b857d"
    );
}

#[test]
fn multi_document_digest_ignores_file_metadata_but_covers_every_art_field() {
    let original = Document::new(2, 2);
    let digest = original.art_digest_v1().unwrap();
    let mut renamed = original.clone();
    renamed.name = "Another filename".into();
    renamed.revision += 42;
    renamed.active = 0;
    renamed.next_layer_id += 10;
    assert_eq!(renamed.art_digest_v1(), Some(digest));
    type Change = Box<dyn Fn(&mut Document)>;
    let changes: Vec<Change> = vec![
        Box::new(|d| d.layers[1].name.push('x')),
        Box::new(|d| d.layers[1].visible = false),
        Box::new(|d| d.layers[1].locked = true),
        Box::new(|d| d.layers[1].alpha_locked = true),
        Box::new(|d| d.layers[1].clipped = true),
        Box::new(|d| d.layers[1].reference = true),
        Box::new(|d| d.layers[1].draft = true),
        Box::new(|d| d.layers[1].opacity = 0.5),
        Box::new(|d| d.layers[1].blend_mode = BlendMode::Multiply),
        Box::new(|d| Arc::make_mut(&mut d.layers[1].pixels)[0] = 9),
        Box::new(|d| {
            d.layers[1].id.0 = 3;
            d.next_layer_id = 4;
        }),
        Box::new(|d| d.layers.swap(0, 1)),
        Box::new(|d| d.selection = Some(MaskCoverage::Uniform(0))),
        Box::new(|d| {
            d.create_layer_mask(1, 255);
        }),
        Box::new(|d| {
            d.group_layers_in_folder(2, "Folder".into()).unwrap();
        }),
    ];
    for change in changes {
        let mut d = original.clone();
        change(&mut d);
        assert!(d.validate());
        assert_ne!(d.art_digest_v1(), Some(digest));
    }
    assert_ne!(Document::new(1, 4).art_digest_v1(), Some(digest));
    let mut zero = original.clone();
    zero.layers[1].opacity = 0.;
    let mut negative = zero.clone();
    negative.layers[1].opacity = -0.;
    assert_eq!(zero.art_digest_v1(), negative.art_digest_v1());
    negative.layers[1].opacity = f32::NAN;
    assert!(negative.art_digest_v1().is_none());
}

#[test]
fn multi_document_digest_expands_coverage_and_distinguishes_presence() {
    let mut uniform = Document::new(3, 2);
    uniform.layers[1].mask =
        Some(LayerMask { enabled: true, coverage: MaskCoverage::Uniform(127) });
    uniform.selection = Some(MaskCoverage::Uniform(128));
    let mut dense = uniform.clone();
    dense.layers[1].mask.as_mut().unwrap().coverage =
        MaskCoverage::Dense(Arc::new(vec![127; 6]));
    dense.selection = Some(MaskCoverage::Dense(Arc::new(vec![128; 6])));
    assert_eq!(uniform.art_digest_v1(), dense.art_digest_v1());
    let expected = dense.art_digest_v1();
    dense.layers[1].mask.as_mut().unwrap().enabled = false;
    assert_ne!(dense.art_digest_v1(), expected);
    dense = uniform.clone();
    dense.layers[1].mask.as_mut().unwrap().coverage =
        MaskCoverage::Uniform(126);
    assert_ne!(dense.art_digest_v1(), expected);
    dense = uniform.clone();
    dense.selection = Some(MaskCoverage::Uniform(255));
    assert_ne!(dense.art_digest_v1(), expected);
    let mut none = Document::new(3, 2);
    let absent = none.art_digest_v1();
    for value in [0, 255] {
        none.selection = Some(MaskCoverage::Uniform(value));
        assert_ne!(none.art_digest_v1(), absent);
    }
    let mut fill = Document::new(3, 2);
    fill.layers[1].pixels = Arc::new(Vec::new());
    fill.layers[1].kind = LayerKind::Fill([1, 2, 3]);
    let first = fill.art_digest_v1().unwrap();
    fill.layers[1].kind = LayerKind::Fill([1, 2, 4]);
    assert_ne!(fill.art_digest_v1(), Some(first));
    fill.layers[0].pixels = Arc::new(Vec::new());
    fill.layers[0].kind = LayerKind::Paper([1, 2, 3]);
    let paper = fill.art_digest_v1().unwrap();
    fill.layers[0].kind = LayerKind::Paper([3, 2, 1]);
    assert_ne!(fill.art_digest_v1(), Some(paper));
}

#[test]
fn multi_document_digest_covers_parent_links_without_changing_other_layer_fields()
 {
    let mut nested = Document::new(2, 2);
    nested.group_layers_in_folder(2, "Inner".into()).unwrap();
    nested.group_layers_in_folder(1 << 2, "Outer".into()).unwrap();
    let expected = nested.art_digest_v1().unwrap();
    let mut sibling = nested.clone();
    sibling.layers[1].parent = Some(sibling.layers[3].id);
    assert!(sibling.validate());
    assert_ne!(sibling.art_digest_v1(), Some(expected));
    assert_eq!(sibling.layers[1].pixels, nested.layers[1].pixels);
}
