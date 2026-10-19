use parfait_core::{Brush, Document, Point, Stroke};
use std::{hint::black_box, time::Instant};

fn main() {
    for size in [24., 128., 512.] {
        for pressure_spacing in [false, true] {
            let mut document = Document::new(1024, 768);
            let brush = Brush {
                size,
                pressure: false,
                pressure_spacing,
                flow: 0.1,
                minimum_spacing: 0.1,
                ..Default::default()
            };
            let start = Instant::now();
            let mut stroke = Stroke::begin(
                &mut document,
                brush,
                Point {
                    x: 256.,
                    y: 300.,
                    pressure: 0.25,
                },
            );
            let mut timings = [0.; 120];
            for (index, timing) in timings.iter_mut().enumerate() {
                let update = Instant::now();
                stroke.extend(
                    &mut document,
                    Point {
                        x: 256. + index as f32 * 4.,
                        y: 300.,
                        pressure: 0.25,
                    },
                );
                if let Some(region) = stroke.take_damage() {
                    black_box(document.composite_region(region));
                }
                *timing = update.elapsed().as_secs_f64() * 1000.;
            }
            let total = start.elapsed().as_secs_f64() * 1000.;
            timings.sort_by(f64::total_cmp);
            println!(
                "size={size:.0} pressure_spacing={pressure_spacing} pressure=0.25 mean_ms={:.4} p95_ms={:.4} max_ms={:.4} scratch_bytes={}",
                total / 120.,
                timings[113],
                timings[119],
                stroke.scratch_capacity_bytes()
            );
        }
    }
}
