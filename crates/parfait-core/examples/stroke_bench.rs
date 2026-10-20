//! Release CPU benchmark: identical dabs, full versus damaged-region composition.
//! Excludes GPU upload, GUI, history snapshots, autosave and device latency.
use parfait_core::{Brush, Document, Point, Stroke};
use std::{hint::black_box, time::Instant};
fn main() {
    let dense_curve =
        std::env::args().any(|argument| argument == "--dense-curve");
    let mut pressure_curve = if dense_curve {
        parfait_core::ResponseCurve {
            points: (0..16)
                .map(|index| {
                    let input = (index as f32 / 15.).powf(1.4);
                    [input, input.sqrt()]
                })
                .collect(),
            controls: None,
        }
    } else {
        Default::default()
    };
    let manual_handles =
        std::env::args().any(|argument| argument == "--bezier");
    if manual_handles {
        pressure_curve.controls = Some(parfait_core::automatic_curve_controls(
            &pressure_curve.points,
        ));
    }
    println!("pressure control points: {}", pressure_curve.points.len());
    println!("explicit Bezier handles: {manual_handles}");
    let gap_mode =
        if std::env::args().any(|argument| argument == "--gap-narrow") {
            parfait_core::GapMode::Narrow
        } else {
            parfait_core::GapMode::Fixed
        };
    let continuous_spray =
        std::env::args().any(|argument| argument == "--spray");
    println!("gap mode: {gap_mode:?}; continuous spray: {continuous_spray}");
    for (width, height, layers) in [(1024, 768, 2), (2048, 2048, 8)] {
        for partial in [false, true] {
            let mut doc = Document::new(width, height);
            while doc.layers.len() < layers {
                doc.add_layer();
            }
            let mut stroke = Stroke::begin(
                &mut doc,
                Brush {
                    size: 24.,
                    pressure_curve: pressure_curve.clone(),
                    gap_mode,
                    continuous_spray,
                    ..Default::default()
                },
                Point { x: 100., y: 100., pressure: 1. },
            );
            black_box(doc.composite()); // initial full render, outside timing
            stroke.take_damage();
            let start = Instant::now();
            let mut bytes = 0;
            for i in 1..=240 {
                stroke.extend(
                    &mut doc,
                    Point {
                        x: 100. + i as f32 * 3.,
                        y: 100. + i as f32 * 0.8,
                        pressure: 0.7 + (i as f32 * 0.1).sin() * 0.3,
                    },
                );
                stroke.spray(&mut doc, 1. / 60., 1.);
                let damage = stroke.take_damage();
                let pixels = if partial {
                    damage.map(|r| doc.composite_region(r)).unwrap_or_default()
                } else {
                    doc.composite()
                };
                bytes += pixels.len();
                black_box(pixels);
            }
            println!(
                "{width}x{height} layers={layers} partial={partial}: {:.3} ms/update; {:.0} bytes/update",
                start.elapsed().as_secs_f64() * 1000. / 240.,
                bytes as f64 / 240.
            );
        }
    }
}
