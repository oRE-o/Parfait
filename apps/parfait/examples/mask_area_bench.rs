#[path = "../src/mask_area.rs"]
mod mask_area;

use parfait_core::{Document, LayerMask, MaskCoverage, PixelRect};
use std::{hint::black_box, sync::Arc, time::Instant};

fn checksum(pixels: &[u8]) -> u64 {
    pixels.iter().map(|value| u64::from(*value)).sum()
}

fn main() {
    let mut document = Document::new(1024, 768);
    document.layers[1].pixels = Arc::new([200, 80, 30, 180].repeat(1024 * 768));
    let original_content = document.layers[1].pixels.clone();
    for (name, coverage) in [
        ("uniform255", MaskCoverage::Uniform(255)),
        ("uniform128", MaskCoverage::Uniform(128)),
        (
            "dense",
            MaskCoverage::Dense(Arc::new(
                (0..1024 * 768)
                    .map(|index| ((index % 1024 * 7 + index / 1024 * 11) % 256) as u8)
                    .collect(),
            )),
        ),
    ] {
        document.layers[1].mask = Some(LayerMask {
            enabled: true,
            coverage: coverage.clone(),
        });
        let full_bounds = PixelRect {
            x0: 0,
            y0: 0,
            x1: 1024,
            y1: 768,
        };
        let mut full = document.composite();
        let untinted = full.clone();
        assert!(mask_area::tint_mask_area(
            &mut full,
            [1024, 768],
            &coverage,
            full_bounds
        ));
        for (index, (before, after)) in untinted
            .as_chunks::<4>()
            .0
            .iter()
            .zip(full.as_chunks::<4>().0.iter())
            .enumerate()
        {
            let alpha = (u32::from(255 - coverage.value(index)) * 128 + 127) / 255;
            for (channel, tint) in [112u32, 96, 255].iter().enumerate() {
                let expected =
                    (u32::from(before[channel]) * (255 - alpha) + tint * alpha + 127) / 255;
                assert_eq!(u32::from(after[channel]), expected);
            }
            assert_eq!(before[3], after[3]);
        }
        for bounds in [
            full_bounds,
            PixelRect {
                x0: 73,
                y0: 91,
                x1: 201,
                y1: 219,
            },
        ] {
            let original = document.composite_region(bounds);
            let mut tinted = original.clone();
            assert!(mask_area::tint_mask_area(
                &mut tinted,
                [1024, 768],
                &coverage,
                bounds
            ));
            let width = (bounds.x1 - bounds.x0) as usize;
            for row in bounds.y0..bounds.y1 {
                let start = ((row * 1024 + bounds.x0) * 4) as usize;
                let local = (row - bounds.y0) as usize * width * 4;
                assert_eq!(
                    &tinted[local..local + width * 4],
                    &full[start..start + width * 4]
                );
            }
            let start = Instant::now();
            let mut compose_checksum = 0;
            for _ in 0..20 {
                compose_checksum += checksum(black_box(&document.composite_region(bounds)));
            }
            let compose_ms = start.elapsed().as_secs_f64() * 1000. / 20.;
            let mut tint_ms = 0.;
            for _ in 0..20 {
                tinted.copy_from_slice(&original);
                let start = Instant::now();
                assert!(mask_area::tint_mask_area(
                    black_box(&mut tinted),
                    [1024, 768],
                    &coverage,
                    bounds
                ));
                tint_ms += start.elapsed().as_secs_f64() * 1000. / 20.;
                black_box(checksum(&tinted));
            }
            assert!(Arc::ptr_eq(&document.layers[1].pixels, &original_content));
            println!(
                "mask={name} width={} height={} composition_mean_ms={compose_ms:.6} tint_only_mean_ms={tint_ms:.6} compose_checksum={compose_checksum} tint_checksum={}",
                bounds.x1 - bounds.x0,
                bounds.y1 - bounds.y0,
                checksum(&tinted)
            );
        }
    }
}
