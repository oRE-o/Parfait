use super::*;
use crate::{BlendMode, History, LayerMask, MaskCoverage, PixelRect};

#[test]
fn brightness_preserves_all_grayscale_alpha_combinations_against_reference() {
    let mut document = Document::new(256, 256);
    let mut input = Vec::with_capacity(256 * 256 * 4);
    let mut expected = Vec::with_capacity(input.capacity());
    for alpha in 0..=255 {
        for gray in 0..=255 {
            input.extend_from_slice(&[
                gray as u8,
                gray as u8,
                gray as u8,
                alpha as u8,
            ]);
            if alpha == 0 {
                expected.extend_from_slice(&[
                    gray as u8, gray as u8, gray as u8, 0,
                ]);
            } else {
                let coverage =
                    (f64::from(alpha) * (1. - f64::from(gray) / 255.)).round();
                expected.extend_from_slice(&[0, 0, 0, coverage as u8]);
            }
        }
    }
    document.layers[1].pixels = Arc::new(input);
    assert!(
        document.convert_layer_pixels(1, RasterConversion::BrightnessToOpacity)
    );
    assert_eq!(document.layers[1].pixels.as_slice(), expected);
    let converted = document.clone();
    assert!(
        !document
            .convert_layer_pixels(1, RasterConversion::BrightnessToOpacity)
    );
    assert_eq!(document, converted);
    assert!(Arc::ptr_eq(
        &document.layers[1].pixels,
        &converted.layers[1].pixels
    ));
}

#[test]
fn color_brightness_and_recolor_have_explicit_straight_alpha_results() {
    let mut document = Document::new(8, 1);
    document.layers[1].pixels = Arc::new(vec![
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 128,
        128, 128, 128, 128, 0, 0, 0, 128, 77, 88, 99, 0, 200, 100, 50, 200,
    ]);
    let before = document.clone();
    assert!(
        document.convert_layer_pixels(1, RasterConversion::BrightnessToOpacity)
    );
    assert_eq!(
        document.layers[1].pixels.as_slice(),
        &[
            0, 0, 0, 179, 0, 0, 0, 105, 0, 0, 0, 226, 0, 0, 0, 0, 0, 0, 0, 64,
            0, 0, 0, 128, 77, 88, 99, 0, 0, 0, 0, 103,
        ]
    );
    document = before.clone();
    document.layers[1].alpha_locked = true;
    assert!(document.convert_layer_pixels(
        1,
        RasterConversion::DrawingColor([19, 79, 239])
    ));
    for (output, source) in document.layers[1]
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(before.layers[1].pixels.as_chunks::<4>().0)
    {
        assert_eq!(output[3], source[3]);
        assert_eq!(
            &output[..3],
            if source[3] == 0 { &source[..3] } else { &[19, 79, 239] }
        );
    }
}

#[test]
fn conversion_preserves_masks_metadata_other_layers_and_partial_composition() {
    for conversion in [
        RasterConversion::BrightnessToOpacity,
        RasterConversion::DrawingColor([8, 9, 10]),
    ] {
        let mut document = Document::new(19, 13);
        document.add_layer();
        document.layers[1].pixels =
            Arc::new([203, 90, 61, 189].repeat(19 * 13));
        document.layers[1].opacity = 0.4;
        document.layers[1].blend_mode = BlendMode::Multiply;
        document.layers[1].clipped = true;
        document.layers[1].mask = Some(LayerMask {
            enabled: true,
            coverage: MaskCoverage::Dense(Arc::new(
                (0..19 * 13).map(|index| (index % 256) as u8).collect(),
            )),
        });
        document.active = 1;
        let before = document.clone();
        assert!(document.convert_layer_pixels(1, conversion));
        assert_eq!(document.revision, before.revision + 1);
        assert_eq!(document.layers[0], before.layers[0]);
        assert_eq!(document.layers[2], before.layers[2]);
        assert!(Arc::ptr_eq(
            &document.layers[0].pixels,
            &before.layers[0].pixels
        ));
        let converted_pixels = document.layers[1].pixels.clone();
        let mut metadata = document.layers[1].clone();
        metadata.pixels = before.layers[1].pixels.clone();
        assert_eq!(metadata, before.layers[1]);
        assert_eq!(document.active, before.active);
        let region = PixelRect { x0: 3, y0: 2, x1: 17, y1: 11 };
        let full = document.composite();
        let partial = document.composite_region(region);
        for row in 0..9 {
            assert_eq!(
                &partial[row * 14 * 4..(row + 1) * 14 * 4],
                &full[((row + 2) * 19 + 3) * 4..((row + 2) * 19 + 17) * 4]
            );
        }
        assert!(Arc::ptr_eq(&document.layers[1].pixels, &converted_pixels));
    }
}

#[test]
fn locked_invalid_and_noop_conversions_preserve_revision_buffers_and_redo() {
    for conversion in [
        RasterConversion::BrightnessToOpacity,
        RasterConversion::DrawingColor([0, 0, 0]),
    ] {
        for blocker in 0..6 {
            let mut document = Document::new(4, 4);
            document.layers[1].pixels = Arc::new([0, 0, 0, 128].repeat(16));
            let layer = if blocker == 0 { 999 } else { 1 };
            if blocker == 1 {
                document.layers[1].locked = true;
            }
            if blocker == 2 {
                document.layers[1].pixels = Arc::new(vec![0]);
            }
            if blocker == 3 {
                document.width = 0;
            }
            if blocker == 4
                && conversion == RasterConversion::BrightnessToOpacity
            {
                document.layers[1].alpha_locked = true;
            }
            let before = document.clone();
            assert!(!document.convert_layer_pixels(layer, conversion));
            assert_eq!(document, before);
            assert!(Arc::ptr_eq(
                &document.layers[1].pixels,
                &before.layers[1].pixels
            ));
        }
        let mut document = Document::new(4, 4);
        document.layers[0].locked = false;
        let mut history = History::default();
        history.begin(&document, "Recolor");
        document
            .convert_layer_pixels(0, RasterConversion::DrawingColor([0, 0, 0]));
        assert!(history.commit(&document));
        assert!(history.undo(&mut document));
        let before = document.clone();
        history.begin(&document, "Unchanged blank layer");
        assert!(!document.convert_layer_pixels(1, conversion));
        assert!(!history.commit(&document));
        assert!(history.can_redo());
        assert_eq!(document, before);
        assert!(Arc::ptr_eq(
            &document.layers[1].pixels,
            &before.layers[1].pixels
        ));
    }
}
