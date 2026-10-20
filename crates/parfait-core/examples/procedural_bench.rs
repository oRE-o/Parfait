use parfait_core::{BlendMode, Document, LayerKind, PixelRect};
use std::{hint::black_box, sync::Arc, time::Instant};

fn timings(
    document: &Document,
    rect: PixelRect,
    expected: &[u8],
) -> (f64, f64) {
    let mut samples = Vec::new();
    for _ in 0..9 {
        let start = Instant::now();
        let output = black_box(document).composite_region(black_box(rect));
        samples.push(start.elapsed().as_secs_f64() * 1000.);
        assert_eq!(output, expected);
        black_box(output);
    }
    samples.sort_by(f64::total_cmp);
    (samples[4], samples[8])
}

fn main() {
    println!(
        "size,through,region,procedural_median_ms,procedural_p95_ms,baked_median_ms,baked_p95_ms,procedural_rgba_bytes,baked_rgba_bytes,row_scratch_bound,checksum"
    );
    for size in [512, 2048] {
        for through in [false, true] {
            let mut document = Document::new_with_paper(size, size);
            document.layers[1].pixels =
                Arc::new([190, 90, 30, 128].repeat((size * size) as usize));
            document
                .create_solid_layer([40, 80, 190], false, "Fill".into())
                .unwrap();
            document.layers[2].opacity = 0.65;
            document.group_layers_in_folder(6, "Group".into()).unwrap();
            document.layers[3].blend_mode =
                if through { BlendMode::Through } else { BlendMode::Multiply };
            document.layers[3].opacity = 0.7;
            let mut baked = document.clone();
            for layer in &mut baked.layers {
                if let Some([red, green, blue]) = layer.kind.solid_color() {
                    layer.kind = LayerKind::Raster;
                    layer.pixels = Arc::new(
                        [red, green, blue, 255].repeat((size * size) as usize),
                    );
                }
            }
            assert!(document.validate() && baked.validate());
            let source_bytes: usize =
                document.layers.iter().map(|layer| layer.pixels.len()).sum();
            let baked_bytes: usize =
                baked.layers.iter().map(|layer| layer.pixels.len()).sum();
            for region in [size, 64] {
                let offset = (size - region) / 2;
                let rect = PixelRect {
                    x0: offset,
                    y0: offset,
                    x1: offset + region,
                    y1: offset + region,
                };
                let expected = baked.composite_region(rect);
                let checksum = expected.iter().fold(
                    14695981039346656037_u64,
                    |hash, byte| {
                        (hash ^ u64::from(*byte)).wrapping_mul(1099511628211)
                    },
                );
                let (procedural_median, procedural_p95) =
                    timings(&document, rect, &expected);
                let (baked_median, baked_p95) =
                    timings(&baked, rect, &expected);
                let bound = document
                    .layers
                    .iter()
                    .filter(|layer| layer.kind != LayerKind::Raster)
                    .count()
                    * region as usize
                    * 4;
                println!(
                    "{size},{through},{region},{procedural_median:.4},{procedural_p95:.4},{baked_median:.4},{baked_p95:.4},{source_bytes},{baked_bytes},{bound},{checksum:016x}"
                );
            }
        }
    }
}
