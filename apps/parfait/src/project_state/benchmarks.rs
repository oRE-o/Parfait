use super::*;
use parfait_core::{LayerMask, MaskCoverage};
use std::{hint::black_box, sync::Arc, time::Instant};

fn fixture(width: u32, height: u32, layers: usize, noisy: bool) -> Document {
    let mut document = Document::new(width, height);
    while document.layers.len() < layers {
        let previous = document.layers.len();
        document.add_layer();
        assert_eq!(document.layers.len(), previous + 1);
    }
    let mut seed = 0x1a2b_3c4du32;
    for (layer_index, layer) in document.layers.iter_mut().enumerate() {
        for (pixel_index, pixel) in Arc::make_mut(&mut layer.pixels)
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            if noisy {
                pixel.copy_from_slice(&seed.to_le_bytes());
            } else {
                pixel.copy_from_slice(&[
                    (pixel_index % width as usize / 32) as u8,
                    (pixel_index / width as usize / 32) as u8,
                    (layer_index * 29) as u8,
                    180,
                ]);
            }
        }
    }
    document.layers[1].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Dense(Arc::new(
            (0..width as usize * height as usize)
                .map(|index| (index % 256) as u8)
                .collect(),
        )),
    });
    assert!(document.validate());
    document
}

fn verify_project(document: &Document, path: &Path) {
    let store = Store::open(path).unwrap();
    let loaded = store.load().unwrap().unwrap().document;
    let mut expected = document.clone();
    expected.revision = loaded.revision;
    assert_eq!(loaded, expected);
    assert!(!store.unclean().unwrap());
}

fn report(case: &str, operation: &str, samples: &[f64], checksum: u64) {
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let maximum = samples.iter().copied().fold(0., f64::max);
    println!(
        "case={case} operation={operation} samples={} mean_ms={mean:.6} max_ms={maximum:.6} checksum={checksum}",
        samples.len()
    );
}

#[test]
#[ignore = "release-only synchronous project save and PNG workload"]
fn synchronous_project_and_png_baseline() {
    if cfg!(debug_assertions) {
        panic!("Run with cargo test --release");
    }
    for (width, height, layers) in [(1024, 768, 2), (2048, 1536, 4)] {
        for noisy in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let mut document = fixture(width, height, layers, noisy);
            let composite = document.composite();
            let checksum = composite.iter().enumerate().fold(
                0u64,
                |checksum, (index, value)| {
                    checksum.wrapping_add(
                        (index as u64 + 1).wrapping_mul(u64::from(*value)),
                    )
                },
            );
            let case = format!("{width}x{height}-{layers}layers-noisy{noisy}");
            println!(
                "case={case} logical_bytes={} snapshot_bytes={}",
                document.logical_pixel_bytes(),
                parfait_storage::snapshot_size(&document).unwrap()
            );
            let existing = directory.path().join("existing.parfait");
            Parfait::write_project_document(&document, &existing).unwrap();
            let mut fresh_times = Vec::new();
            let mut overwrite_times = Vec::new();
            let mut export_times = Vec::new();
            for iteration in 0..3 {
                document.touch();
                let fresh =
                    directory.path().join(format!("fresh-{iteration}.parfait"));
                let start = Instant::now();
                Parfait::write_project_document(black_box(&document), &fresh)
                    .unwrap();
                fresh_times.push(start.elapsed().as_secs_f64() * 1000.);
                verify_project(&document, &fresh);
                std::fs::remove_file(fresh).unwrap();
                let start = Instant::now();
                Parfait::write_project_document(
                    black_box(&document),
                    &existing,
                )
                .unwrap();
                overwrite_times.push(start.elapsed().as_secs_f64() * 1000.);
                verify_project(&document, &existing);
                let png =
                    directory.path().join(format!("export-{iteration}.png"));
                let start = Instant::now();
                image::save_buffer_with_format(
                    &png,
                    &black_box(&document).composite(),
                    width,
                    height,
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .unwrap();
                export_times.push(start.elapsed().as_secs_f64() * 1000.);
                assert_eq!(
                    image::open(&png).unwrap().into_rgba8().as_raw(),
                    &composite
                );
                std::fs::remove_file(png).unwrap();
            }
            report(&case, "fresh_project", &fresh_times, checksum);
            report(&case, "overwrite_project", &overwrite_times, checksum);
            report(&case, "png_composite_write", &export_times, checksum);
        }
    }
}
