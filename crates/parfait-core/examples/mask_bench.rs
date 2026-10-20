use parfait_core::{Document, LayerMask, MaskCoverage, PixelRect};
use std::{hint::black_box, sync::Arc, time::Instant};

fn main() {
    let mut document = Document::new(1024, 768);
    while document.layers.len() < 8 {
        document.add_layer();
    }
    for (index, layer) in document.layers.iter_mut().enumerate() {
        layer.opacity = 0.7;
        for (offset, pixel) in Arc::make_mut(&mut layer.pixels)
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            *pixel = [
                (offset * 31 + index * 7) as u8,
                (offset * 19) as u8,
                (index * 29) as u8,
                (offset * 53 + index * 13) as u8,
            ];
        }
    }
    for clipped in [false, true] {
        for kind in ["uniform255", "uniform128", "dense"] {
            for (index, layer) in document.layers.iter_mut().enumerate() {
                layer.clipped = clipped && index % 4 != 0;
                layer.mask = Some(LayerMask {
                    enabled: true,
                    coverage: match kind {
                        "uniform255" => MaskCoverage::Uniform(255),
                        "uniform128" => MaskCoverage::Uniform(128),
                        _ => MaskCoverage::Dense(Arc::new(
                            (0..1024 * 768)
                                .map(|offset| (offset * 13 + index * 71) as u8)
                                .collect(),
                        )),
                    },
                });
            }
            let full = document.composite();
            let mut baked = document.clone();
            for index in 0..baked.layers.len() {
                baked.layers[index].locked = false;
                assert!(baked.apply_layer_mask(index));
            }
            assert_eq!(baked.composite(), full);
            for rect in [
                PixelRect { x0: 0, y0: 0, x1: 1024, y1: 768 },
                PixelRect { x0: 301, y0: 251, x1: 429, y1: 379 },
            ] {
                let expected: Vec<_> = (rect.y0..rect.y1)
                    .flat_map(|row| {
                        let start =
                            ((row * document.width + rect.x0) * 4) as usize;
                        full[start..start + ((rect.x1 - rect.x0) * 4) as usize]
                            .iter()
                            .copied()
                    })
                    .collect();
                assert_eq!(document.composite_region(rect), expected);
                let start = Instant::now();
                let mut checksum = 0u64;
                for _ in 0..10 {
                    let pixels = document.composite_region(black_box(rect));
                    checksum = checksum.wrapping_add(
                        pixels.iter().map(|byte| u64::from(*byte)).sum::<u64>(),
                    );
                    black_box(pixels);
                }
                println!(
                    "clipped={clipped} mask={kind} {}x{} mean_ms={:.4} output_bytes={} checksum={checksum}",
                    rect.x1 - rect.x0,
                    rect.y1 - rect.y0,
                    start.elapsed().as_secs_f64() * 100.,
                    expected.len()
                );
            }
        }
    }
}
