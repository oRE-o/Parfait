use parfait_core::{BlendMode, Document, PixelRect};
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
        for (index, layer) in document.layers.iter_mut().enumerate() {
            layer.clipped = clipped && index % 4 != 0;
            layer.blend_mode = if clipped && index % 2 == 1 {
                BlendMode::Multiply
            } else {
                BlendMode::Normal
            };
        }
        let full = document.composite();
        for rect in [
            PixelRect { x0: 0, y0: 0, x1: 1024, y1: 768 },
            PixelRect { x0: 301, y0: 251, x1: 429, y1: 379 },
        ] {
            let expected: Vec<_> = (rect.y0..rect.y1)
                .flat_map(|row| {
                    let start = ((row * document.width + rect.x0) * 4) as usize;
                    full[start..start + ((rect.x1 - rect.x0) * 4) as usize]
                        .iter()
                        .copied()
                })
                .collect();
            assert_eq!(document.composite_region(rect), expected);
            let start = Instant::now();
            let mut checksum = 0u64;
            for _ in 0..20 {
                let pixels = document.composite_region(black_box(rect));
                checksum = checksum.wrapping_add(
                    pixels.iter().map(|byte| u64::from(*byte)).sum::<u64>(),
                );
                black_box(pixels);
            }
            println!(
                "clipped={clipped} {}x{} mean_ms={:.4} output_bytes={} checksum={checksum}",
                rect.x1 - rect.x0,
                rect.y1 - rect.y0,
                start.elapsed().as_secs_f64() * 50.,
                expected.len()
            );
        }
    }
}
