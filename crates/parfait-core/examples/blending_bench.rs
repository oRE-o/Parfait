use parfait_core::{Brush, BrushOperation, Document, Point, Stroke};
use std::{hint::black_box, sync::Arc, time::Instant};

fn main() {
    let alpha_locked = std::env::args().any(|argument| argument == "--alpha-lock");
    if alpha_locked {
        println!("alpha_locked=true");
    }
    for size in [24., 128., 512.] {
        for operation in [
            BrushOperation::Paint,
            BrushOperation::Blend,
            BrushOperation::Blur,
            BrushOperation::Smudge,
        ] {
            let mut document = Document::new(1024, 768);
            document.layers[1].alpha_locked = alpha_locked;
            for (index, pixel) in Arc::make_mut(&mut document.layers[1].pixels)
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .enumerate()
            {
                pixel.copy_from_slice(&[(index % 251) as u8, (index / 1024 % 193) as u8, 160, 180]);
            }
            let brush = Brush {
                operation,
                size,
                pressure: false,
                blur_automatic: false,
                blur_radius: 32.,
                flow: 0.5,
                ..Default::default()
            };
            let start = Instant::now();
            let mut stroke = Stroke::begin(
                &mut document,
                brush,
                Point {
                    x: 256.,
                    y: 300.,
                    pressure: 1.,
                },
            );
            let updates = 120;
            let first_ms = start.elapsed().as_secs_f64() * 1000.;
            let mut timings = [0.; 120];
            let mut bytes = 0;
            for (index, timing) in timings.iter_mut().enumerate() {
                let update_start = Instant::now();
                stroke.extend(
                    &mut document,
                    Point {
                        x: 256. + index as f32 * 4.,
                        y: 300. + (index as f32 * 0.1).sin() * 16.,
                        pressure: 1.,
                    },
                );
                if let Some(rect) = stroke.take_damage() {
                    bytes += black_box(document.composite_region(rect)).len();
                }
                *timing = update_start.elapsed().as_secs_f64() * 1000.;
            }
            let elapsed = start.elapsed().as_secs_f64() * 1000.;
            timings.sort_by(f64::total_cmp);
            println!(
                "{operation:?} size={size:.0} updates={updates} ms/update={:.4} partial_bytes/update={} scratch_bytes={} first_ms={first_ms:.4} p95_ms={:.4} max_ms={:.4}",
                elapsed / updates as f64,
                bytes / updates,
                stroke.scratch_capacity_bytes(),
                timings[113],
                timings[119]
            );
            black_box(document);
        }
    }
}
