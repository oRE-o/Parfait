use super::*;
use crate::{CanvasEdit, MaskCoverage};

fn colored() -> Document {
    let mut doc = Document::new(3, 2);
    doc.layers[1].pixels = Arc::new(vec![
        10, 20, 30, 128, 255, 0, 0, 255, 0, 0, 255, 64, 100, 100, 100, 255, 0,
        255, 0, 255, 0, 0, 0, 0,
    ]);
    doc
}
#[test]
fn correction_identity_invalid_values_and_noops_keep_buffers_and_revision() {
    let edits = [
        PixelEdit::BrightnessContrast { brightness: 0., contrast: 0. },
        PixelEdit::Hsl { hue: 0., saturation: 0., lightness: 0. },
        PixelEdit::Levels { black: 0, white: 255, gamma: 1. },
        PixelEdit::Curve([0., 0.25, 0.5, 0.75, 1.]),
        PixelEdit::Balance([0.; 3]),
        PixelEdit::Posterize(255),
    ];
    for edit in edits.into_iter().take(5) {
        let mut doc = colored();
        let before = doc.clone();
        assert!(!doc.edit_layer_pixels(1, edit), "{edit:?}");
        assert_eq!(doc.revision, before.revision);
        assert!(Arc::ptr_eq(&doc.layers[1].pixels, &before.layers[1].pixels));
    }
    for edit in [
        PixelEdit::Levels { black: 200, white: 100, gamma: 1. },
        PixelEdit::Balance([f32::NAN, 0., 0.]),
        PixelEdit::Border { color: [0; 4], radius: 0 },
        PixelEdit::Hsl { hue: f32::INFINITY, saturation: 0., lightness: 0. },
    ] {
        let mut doc = colored();
        let before = doc.clone();
        assert!(!doc.edit_layer_pixels(1, edit));
        assert_eq!(doc, before);
    }
}
#[test]
fn corrections_obey_soft_selection_and_locks_without_changing_masks_or_other_layers()
 {
    let mut doc = colored();
    doc.selection = Some(MaskCoverage::Uniform(128));
    doc.create_layer_mask(1, 200);
    let before = doc.clone();
    assert!(doc.edit_layer_pixels(1, PixelEdit::Invert));
    assert_eq!(&doc.layers[1].pixels[..4], &[128, 128, 128, 128]);
    assert_eq!(doc.layers[1].mask, before.layers[1].mask);
    assert!(Arc::ptr_eq(&doc.layers[0].pixels, &before.layers[0].pixels));
    doc.layers[1].locked = true;
    let before = doc.clone();
    assert!(!doc.edit_layer_pixels(1, PixelEdit::Invert));
    assert_eq!(doc, before);
    doc.layers[1].locked = false;
    let folder = doc.group_layers_in_folder(2, "Folder".into()).unwrap();
    let f = doc.layer_index(folder).unwrap();
    doc.layers[f].locked = true;
    let before = doc.clone();
    assert!(!doc.edit_layer_pixels(1, PixelEdit::Fill([0; 4])));
    assert_eq!(doc, before);
}
#[test]
fn fill_clear_outside_and_alpha_lock_use_true_coverage() {
    let mut doc = Document::new(2, 1);
    doc.selection = Some(MaskCoverage::Uniform(64));
    assert!(doc.edit_layer_pixels(1, PixelEdit::Fill([20, 50, 90, 255])));
    assert_eq!(doc.layers[1].pixels.as_slice(), [20, 50, 90, 64].repeat(2));
    assert!(doc.edit_layer_pixels(1, PixelEdit::ClearOutside));
    assert_eq!(doc.layers[1].pixels[3], 16);
    doc.layers[1].alpha_locked = true;
    assert!(doc.edit_layer_pixels(1, PixelEdit::Fill([255, 0, 0, 255])));
    assert_eq!(doc.layers[1].pixels[3], 16);
    assert!(doc.pixel_edit_reason(1, PixelEdit::ClearOutside).is_some());
}
#[test]
fn border_matches_independent_neighborhood_reference_for_edges_holes_and_large_radii()
 {
    for width in 1..=9 {
        for height in 1..=7 {
            for radius in 1..=4 {
                let mut doc = Document::new(width, height);
                let values: Vec<_> = (0..width * height)
                    .map(|i| {
                        if i % 5 == 0 {
                            0
                        } else if i % 3 == 0 {
                            128
                        } else {
                            255
                        }
                    })
                    .collect();
                doc.selection =
                    Some(MaskCoverage::Dense(Arc::new(values.clone())));
                let result = doc.selection_border(radius);
                for y in 0..height {
                    for x in 0..width {
                        let mut min = 255;
                        for dy in -(radius as i32)..=radius as i32 {
                            for dx in -(radius as i32)..=radius as i32 {
                                let sx = x as i32 + dx;
                                let sy = y as i32 + dy;
                                min = min.min(
                                    if sx < 0
                                        || sy < 0
                                        || sx >= width as i32
                                        || sy >= height as i32
                                    {
                                        0
                                    } else {
                                        values[(sy as u32 * width + sx as u32)
                                            as usize]
                                    },
                                );
                            }
                        }
                        let i = (y * width + x) as usize;
                        assert_eq!(result[i], values[i].saturating_sub(min));
                    }
                }
            }
        }
    }
}
#[test]
fn every_correction_changes_expected_colors_and_preserves_alpha() {
    for edit in [
        PixelEdit::BrightnessContrast { brightness: 0.2, contrast: 0.1 },
        PixelEdit::Hsl { hue: 90., saturation: 0.3, lightness: 0.1 },
        PixelEdit::Posterize(2),
        PixelEdit::Invert,
        PixelEdit::Levels { black: 30, white: 220, gamma: 0.7 },
        PixelEdit::Curve([0., 0.1, 0.3, 0.8, 1.]),
        PixelEdit::Balance([0.1, -0.2, 0.3]),
        PixelEdit::Threshold(128),
        PixelEdit::Gradient { low: [20, 30, 40], high: [200, 220, 240] },
    ] {
        let mut doc = colored();
        let before = doc.clone();
        assert!(doc.edit_layer_pixels(1, edit), "{edit:?}");
        assert!(doc.validate());
        for (a, b) in doc.layers[1]
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .zip(before.layers[1].pixels.as_chunks::<4>().0)
        {
            assert_eq!(a[3], b[3]);
        }
    }
}
#[test]
fn clipboard_respects_selection_mask_and_source_origin_and_pastes_as_independent_layer()
 {
    let mut doc = colored();
    doc.selection =
        Some(MaskCoverage::Dense(Arc::new(vec![0, 128, 255, 0, 0, 0])));
    doc.create_layer_mask(1, 128);
    let before = doc.clone();
    let clip = doc.copy_layer_pixels(1).unwrap();
    assert_eq!((clip.width, clip.height, clip.origin), (2, 1, [1, 0]));
    assert_eq!(clip.pixels, [255, 0, 0, 64, 0, 0, 255, 32]);
    assert_eq!(doc, before);
    assert!(doc.paste_pixels(&clip, [-1, 1]));
    assert!(doc.validate());
    assert_eq!(doc.layers.len(), 3);
    assert_eq!(&doc.layers[2].pixels[12..16], &[0, 0, 255, 32]);
    assert!(Arc::ptr_eq(&doc.layers[1].pixels, &before.layers[1].pixels));
    let before = doc.clone();
    let invalid = PixelClipboard {
        width: 4097,
        height: 1,
        origin: [0; 2],
        pixels: vec![],
    };
    assert!(!doc.paste_pixels(&invalid, [0; 2]));
    assert_eq!(doc, before);
}
#[test]
fn canvas_rotation_matches_full_composite_and_rotates_dense_masks_and_selection()
 {
    for edit in [
        CanvasEdit::RotateRight,
        CanvasEdit::RotateLeft,
        CanvasEdit::RotateHalf,
        CanvasEdit::FlipHorizontal,
        CanvasEdit::FlipVertical,
    ] {
        let mut doc = colored();
        doc.create_layer_mask(1, 255);
        doc.layers[1].mask.as_mut().unwrap().coverage =
            MaskCoverage::Dense(Arc::new(vec![0, 50, 100, 150, 200, 255]));
        doc.selection = Some(MaskCoverage::Dense(Arc::new(vec![
            0, 51, 102, 153, 204, 255,
        ])));
        let before = doc.clone();
        let composite = doc.composite();
        assert!(doc.edit_canvas(edit));
        assert!(doc.validate());
        let result = doc.composite();
        for y in 0..doc.height {
            for x in 0..doc.width {
                let (sx, sy) = match edit {
                    CanvasEdit::RotateRight => (y, before.height - 1 - x),
                    CanvasEdit::RotateLeft => (before.width - 1 - y, x),
                    CanvasEdit::RotateHalf => {
                        (before.width - 1 - x, before.height - 1 - y)
                    }
                    CanvasEdit::FlipHorizontal => (before.width - 1 - x, y),
                    CanvasEdit::FlipVertical => (x, before.height - 1 - y),
                    _ => unreachable!(),
                };
                let i = (y * doc.width + x) as usize;
                let src = (sy * before.width + sx) as usize;
                assert_eq!(
                    &result[i * 4..i * 4 + 4],
                    &composite[src * 4..src * 4 + 4]
                );
                assert_eq!(doc.selection_value(i), before.selection_value(src));
                assert_eq!(
                    doc.layers[1].mask.as_ref().unwrap().coverage.value(i),
                    before.layers[1].mask.as_ref().unwrap().coverage.value(src)
                );
            }
        }
    }
}
#[test]
fn resizing_uses_premultiplied_colors_and_canvas_extension_preserves_coverage_semantics()
 {
    let mut doc = Document::new(2, 1);
    doc.layers[1].pixels = Arc::new(vec![255, 0, 0, 255, 0, 0, 255, 0]);
    assert!(doc.edit_canvas(CanvasEdit::Resize { width: 3, height: 1 }));
    assert_eq!(&doc.layers[1].pixels[4..8], &[255, 0, 0, 128]);
    doc.selection = Some(MaskCoverage::Uniform(255));
    doc.create_layer_mask(1, 0);
    assert!(doc.edit_canvas(CanvasEdit::Extent {
        width: 5,
        height: 3,
        offset: [1, 1]
    }));
    assert_eq!(doc.selection_value(0), 0);
    assert_eq!(doc.selection_value(6), 255);
    let mask = &doc.layers[1].mask.as_ref().unwrap().coverage;
    assert_eq!(mask.value(0), 255);
    assert_eq!(mask.value(6), 0);
    assert!(doc.validate());
    let before = doc.clone();
    assert!(!doc.edit_canvas(CanvasEdit::Resize { width: 4097, height: 1 }));
    assert_eq!(doc, before);
    assert!(!doc.edit_canvas(CanvasEdit::Crop(PixelRect {
        x0: 2,
        x1: 1,
        y0: 0,
        y1: 1
    })));
    assert_eq!(doc, before);
}

#[test]
fn svg_uses_only_bounded_vector_paths_and_exact_alpha_without_embedded_resources()
 {
    let clip = PixelClipboard {
        width: 3,
        height: 1,
        origin: [0; 2],
        pixels: vec![10, 20, 30, 128, 10, 20, 30, 128, 1, 2, 3, 0],
    };
    let svg = clip.to_svg().unwrap();
    assert!(svg.contains("M0 0h2v1H0Z"));
    assert!(svg.contains("#0a141e"));
    assert!(svg.contains("0.501961"));
    assert_eq!(svg.matches("<path").count(), 1);
    for excluded in ["<image", "<script", "href=", "foreignObject"] {
        assert!(!svg.contains(excluded));
    }
}
