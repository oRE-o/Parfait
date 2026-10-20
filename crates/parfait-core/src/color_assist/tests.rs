use super::*;
fn line_art() -> Document {
    let mut doc = Document::new(7, 7);
    let pixels = Arc::make_mut(&mut doc.layers[1].pixels);
    for y in 1..=5 {
        for x in 1..=5 {
            if x == 1 || x == 5 || y == 1 || y == 5 {
                let i = (y * 7 + x) * 4;
                pixels[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
    doc
}
fn options() -> RegionColors {
    RegionColors {
        threshold: 220,
        minimum_area: 4,
        include_open_regions: false,
        palette: vec![[100, 150, 200]],
    }
}
#[test]
fn region_coloring_fills_closed_interior_and_retains_original_buffers() {
    let mut doc = line_art();
    let before = doc.clone();
    assert!(doc.color_regions(&options(), None));
    assert!(doc.validate());
    assert_eq!(doc.layers.len(), 3);
    let pixels = &doc.layers[2].pixels;
    for y in 0..7 {
        for x in 0..7 {
            let i = (y * 7 + x) * 4;
            assert_eq!(
                pixels[i + 3],
                if (2..=4).contains(&x) && (2..=4).contains(&y) {
                    255
                } else {
                    0
                }
            );
        }
    }
    assert!(Arc::ptr_eq(&doc.layers[1].pixels, &before.layers[1].pixels));
    assert_eq!(&doc.composite()[(3 * 7 + 3) * 4..][..4], &[100, 150, 200, 255]);
}
#[test]
fn hint_colors_and_selection_coverage_are_used_and_invalid_options_are_atomic()
{
    let mut doc = line_art();
    doc.selection = Some(crate::MaskCoverage::Uniform(128));
    let mut hint = PixelClipboard {
        width: 7,
        height: 7,
        origin: [0; 2],
        pixels: vec![0; 7 * 7 * 4],
    };
    hint.pixels[(3 * 7 + 3) * 4..][..4].copy_from_slice(&[220, 30, 170, 255]);
    assert!(doc.color_regions(&options(), Some(&hint)));
    assert_eq!(
        &doc.layers[2].pixels[(3 * 7 + 3) * 4..][..4],
        &[220, 30, 170, 128]
    );
    let before = doc.clone();
    let invalid = RegionColors { palette: vec![], ..options() };
    assert!(!doc.color_regions(&invalid, None));
    assert_eq!(doc, before);
    let mut empty = Document::new(4, 4);
    let before = empty.clone();
    assert!(!empty.color_regions(&options(), None));
    assert_eq!(empty, before);
}
#[test]
fn statistical_color_match_and_shading_preserve_alpha_and_respect_locks() {
    let mut doc = Document::new(2, 1);
    doc.layers[1].pixels = Arc::new([200, 100, 50, 128].repeat(2));
    let reference = PixelClipboard {
        width: 1,
        height: 1,
        origin: [0; 2],
        pixels: vec![20, 40, 230, 255],
    };
    assert!(doc.match_layer_colors(1, &reference));
    assert_eq!(doc.layers[1].pixels.as_slice(), [20, 40, 230, 128].repeat(2));
    let before = doc.clone();
    assert!(!doc.shade_layer(1, 45., 0.));
    assert_eq!(doc, before);
    assert!(doc.shade_layer(1, -45., 0.8));
    assert_eq!(doc.layers[1].pixels[3], 128);
    doc.layers[1].locked = true;
    let before = doc.clone();
    assert!(!doc.match_layer_colors(1, &reference));
    assert!(!doc.shade_layer(1, 45., 0.5));
    assert_eq!(doc, before);
}
#[test]
fn back_projection_keeps_constant_color_alpha_and_exact_output_dimensions() {
    let mut doc = Document::new(3, 2);
    doc.layers[1].pixels = Arc::new([80, 120, 200, 128].repeat(6));
    assert!(doc.edit_canvas(crate::CanvasEdit::SuperResolve {
        width: 6,
        height: 4,
        iterations: 4,
        strength: 0.5
    }));
    assert_eq!((doc.width, doc.height), (6, 4));
    assert!(doc.validate());
    assert_eq!(doc.layers[1].pixels.as_slice(), [80, 120, 200, 128].repeat(24));
    let before = doc.clone();
    assert!(!doc.edit_canvas(crate::CanvasEdit::SuperResolve {
        width: 6,
        height: 4,
        iterations: 4,
        strength: 0.5
    }));
    assert_eq!(doc, before);
}
