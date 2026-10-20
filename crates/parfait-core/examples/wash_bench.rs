use parfait_core::{
    Brush, BrushOperation, Document, OpacityMode, Point, Stroke,
};
use std::{hint::black_box, sync::Arc, time::Instant};

fn main() {
    let alpha_locked =
        std::env::args().any(|argument| argument == "--alpha-lock");
    if alpha_locked {
        println!("alpha_locked=true");
    }
    for size in [24., 128., 512.] {
        for operation in [BrushOperation::Paint, BrushOperation::Erase] {
            for opacity_mode in [OpacityMode::PerDab, OpacityMode::PerStroke] {
                let mut document = Document::new(1024, 768);
                document.layers[1].alpha_locked = alpha_locked;
                for (index, pixel) in
                    Arc::make_mut(&mut document.layers[1].pixels)
                        .as_chunks_mut::<4>()
                        .0
                        .iter_mut()
                        .enumerate()
                {
                    *pixel = [(index % 251) as u8, 90, 210, 180];
                }
                let snapshot = document.clone();
                let brush = Brush {
                    size,
                    operation,
                    opacity_mode,
                    pressure: false,
                    opacity: 0.4,
                    flow: 0.25,
                    ..Default::default()
                };
                let start = Instant::now();
                let mut stroke = Stroke::begin(
                    &mut document,
                    brush,
                    Point { x: 256., y: 300., pressure: 1. },
                );
                let first_ms = start.elapsed().as_secs_f64() * 1000.;
                let mut timings = [0.; 120];
                for (index, timing) in timings.iter_mut().enumerate() {
                    let update = Instant::now();
                    stroke.extend(
                        &mut document,
                        Point {
                            x: 256. + index as f32 * 4.,
                            y: 300. + (index as f32 * 0.1).sin() * 16.,
                            pressure: 1.,
                        },
                    );
                    if let Some(region) = stroke.take_damage() {
                        black_box(document.composite_region(region));
                    }
                    *timing = update.elapsed().as_secs_f64() * 1000.;
                }
                let mean = start.elapsed().as_secs_f64() * 1000. / 120.;
                timings.sort_by(f64::total_cmp);
                println!(
                    "{operation:?} {opacity_mode:?} size={size:.0} mean_ms={mean:.4} first_ms={first_ms:.4} p95_ms={:.4} max_ms={:.4} scratch_bytes={}",
                    timings[113],
                    timings[119],
                    stroke.scratch_capacity_bytes()
                );
                black_box((snapshot, document));
            }
        }
    }
}
