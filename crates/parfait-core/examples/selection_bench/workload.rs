use parfait_core::{
    Brush, BrushOperation, Document, OpacityMode, Point, Stroke,
};
use std::{hint::black_box, sync::Arc, time::Instant};

pub fn fixture() -> Document {
    let mut document = Document::new(512, 512);
    document.layers[1].pixels = Arc::new(
        (0..512 * 512)
            .flat_map(|index| {
                [
                    (index * 17) as u8,
                    (index * 31) as u8,
                    (index * 11) as u8,
                    ((index * 23) % 256) as u8,
                ]
            })
            .collect(),
    );
    document
}

pub fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(14695981039346656037, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(1099511628211)
    })
}

pub fn run(document: &Document, verify: impl Fn(&Document)) {
    for operation in [
        BrushOperation::Paint,
        BrushOperation::Erase,
        BrushOperation::Blend,
        BrushOperation::Blur,
        BrushOperation::Smudge,
    ] {
        for opacity_mode in [OpacityMode::PerDab, OpacityMode::PerStroke] {
            let mut elapsed = Vec::new();
            let mut expected = None;
            let mut scratch = 0;
            for _ in 0..25 {
                let mut sample = document.clone();
                let brush = Brush {
                    operation,
                    opacity_mode,
                    size: 48.,
                    pressure: false,
                    opacity: 0.65,
                    flow: 0.6,
                    ..Default::default()
                };
                let started = Instant::now();
                let mut stroke = Stroke::begin(
                    black_box(&mut sample),
                    brush,
                    Point { x: 96., y: 192., pressure: 1. },
                );
                for index in 1..=64 {
                    stroke.extend(
                        &mut sample,
                        Point {
                            x: 96. + index as f32 * 4.,
                            y: 192. + index as f32 * 0.5,
                            pressure: 1.,
                        },
                    );
                }
                elapsed.push(started.elapsed().as_secs_f64() * 1000.);
                scratch = scratch.max(stroke.scratch_capacity_bytes());
                let hash = checksum(&sample.layers[1].pixels);
                assert_eq!(*expected.get_or_insert(hash), hash);
                verify(&sample);
            }
            elapsed.sort_by(f64::total_cmp);
            println!(
                "{operation:?},{opacity_mode:?},{:.6},{:.6},{scratch},{}",
                elapsed[12],
                elapsed[23],
                expected.unwrap()
            );
        }
    }
}
