use parfait_core::{Document, RasterConversion};
use std::{hint::black_box, sync::Arc, time::Instant};

fn main() {
    let mut source = Document::new(1024, 768);
    for (offset, pixel) in Arc::make_mut(&mut source.layers[1].pixels)
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        *pixel = [
            (offset * 31) as u8,
            (offset * 17) as u8,
            (offset * 9) as u8,
            (offset * 13) as u8,
        ];
    }
    for conversion in [
        RasterConversion::BrightnessToOpacity,
        RasterConversion::DrawingColor([19, 110, 230]),
    ] {
        for shared in [true, false] {
            let mut times = Vec::with_capacity(20);
            let mut checksum = 0u64;
            for _ in 0..20 {
                let mut document = source.clone();
                if !shared {
                    document.layers[1].pixels = Arc::new(source.layers[1].pixels.as_ref().clone());
                }
                let start = Instant::now();
                assert!(document.convert_layer_pixels(1, black_box(conversion)));
                times.push(start.elapsed().as_secs_f64() * 1000.);
                for (output, input) in document.layers[1]
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(source.layers[1].pixels.as_chunks::<4>().0)
                {
                    let expected = if input[3] == 0 {
                        *input
                    } else {
                        match conversion {
                            RasterConversion::DrawingColor(color) => {
                                [color[0], color[1], color[2], input[3]]
                            }
                            RasterConversion::BrightnessToOpacity => {
                                let gray = ((f64::from(input[0]) * 299.
                                    + f64::from(input[1]) * 587.
                                    + f64::from(input[2]) * 114.)
                                    / 1000.)
                                    .round();
                                [
                                    0,
                                    0,
                                    0,
                                    (f64::from(input[3]) * (255. - gray) / 255.).round() as u8,
                                ]
                            }
                        }
                    };
                    assert_eq!(*output, expected);
                    checksum = checksum
                        .wrapping_add(output.iter().map(|byte| u64::from(*byte)).sum::<u64>());
                }
                let before = document.clone();
                assert!(!document.convert_layer_pixels(1, conversion));
                assert_eq!(document.revision, before.revision);
                assert!(Arc::ptr_eq(
                    &document.layers[1].pixels,
                    &before.layers[1].pixels
                ));
                black_box(document);
            }
            times.sort_by(f64::total_cmp);
            println!(
                "{conversion:?} shared={shared} mean_ms={:.6} p95_ms={:.6} checksum={checksum}",
                times.iter().sum::<f64>() / times.len() as f64,
                times[18]
            );
        }
    }
}
