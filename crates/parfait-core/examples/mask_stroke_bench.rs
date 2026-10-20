use parfait_core::{
    Brush, Document, MaskCoverage, OpacityMode, PixelRect, Point, Stroke,
};
use std::{hint::black_box, sync::Arc, time::Instant};

fn main() {
    for size in [32., 128., 512.] {
        for mode in [OpacityMode::PerDab, OpacityMode::PerStroke] {
            for erase in [false, true] {
                let mut document = Document::new(1024, 768);
                document.layers[1].pixels =
                    Arc::new([80, 120, 220, 255].repeat(1024 * 768));
                document.create_layer_mask(1, 127);
                let before = document.clone();
                let brush = Brush {
                    size,
                    erase,
                    opacity: 0.65,
                    flow: 0.4,
                    pressure: false,
                    smoothing: 0.,
                    opacity_mode: mode,
                    ..Default::default()
                };
                let point = |step: usize| Point {
                    x: 128. + step as f32 * 12.,
                    y: 384.,
                    pressure: 1.,
                };
                let begin = Instant::now();
                let mut stroke =
                    Stroke::begin_mask(&mut document, brush, point(0)).unwrap();
                let begin_ms = begin.elapsed().as_secs_f64() * 1000.;
                let mut updates = Vec::with_capacity(64);
                let mut damage = stroke.take_damage().unwrap();
                for step in 1..=64 {
                    let start = Instant::now();
                    stroke.extend(&mut document, point(step));
                    updates.push(start.elapsed().as_secs_f64() * 1000.);
                    if let Some(region) = stroke.take_damage() {
                        damage = damage.union(region);
                    }
                }
                assert!(Arc::ptr_eq(
                    &document.layers[1].pixels,
                    &before.layers[1].pixels
                ));
                let full = document.composite();
                let region = document.composite_region(damage);
                let region_width = damage.x1 - damage.x0;
                for row in damage.y0..damage.y1 {
                    let start =
                        ((row * document.width + damage.x0) * 4) as usize;
                    let local = ((row - damage.y0) * region_width * 4) as usize;
                    assert_eq!(
                        &region[local..local + region_width as usize * 4],
                        &full[start..start + region_width as usize * 4]
                    );
                }
                let mask = document.layers[1].mask.as_ref().unwrap();
                let checksum = match &mask.coverage {
                    MaskCoverage::Uniform(value) => {
                        u64::from(*value) * 1024 * 768
                    }
                    MaskCoverage::Dense(values) => {
                        values.iter().map(|value| u64::from(*value)).sum()
                    }
                };
                black_box(checksum);
                updates.sort_by(f64::total_cmp);
                let scratch = stroke.scratch_capacity_bytes();
                drop(stroke);
                document.apply_layer_mask(1);
                assert_eq!(document.composite(), full);
                let bounds: PixelRect = damage;
                println!(
                    "size={size:.0} mode={mode:?} erase={erase} begin_ms={begin_ms:.6} p50_ms={:.6} p95_ms={:.6} max_ms={:.6} scratch_bytes={scratch} checksum={checksum} bounds={bounds:?}",
                    updates[32], updates[60], updates[63]
                );
            }
        }
    }
}
