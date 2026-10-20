//! CPU-only populated-layer composition; excludes UI, GPU, history and autosave.
use parfait_core::{BlendMode, Document, PixelRect};
use std::{hint::black_box, sync::Arc, time::Instant};
fn main() {
    for mode in [BlendMode::Normal, BlendMode::Multiply, BlendMode::SoftLight] {
        let mut doc = Document::new(1024, 768);
        while doc.layers.len() < 8 {
            doc.add_layer();
        }
        for (i, layer) in doc.layers.iter_mut().enumerate() {
            layer.blend_mode = mode;
            layer.opacity = 0.8;
            for p in Arc::make_mut(&mut layer.pixels)
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
            {
                p.copy_from_slice(&[40 + i as u8 * 20, 160, 90, 180]);
            }
        }
        for partial in [false, true] {
            let rect = if partial {
                PixelRect { x0: 100, y0: 100, x1: 132, y1: 132 }
            } else {
                PixelRect { x0: 0, y0: 0, x1: 1024, y1: 768 }
            };
            black_box(doc.composite_region(rect));
            let start = Instant::now();
            for _ in 0..30 {
                black_box(doc.composite_region(rect));
            }
            println!(
                "{mode:?} partial={partial}: {:.3} ms/update",
                start.elapsed().as_secs_f64() * 1000. / 30.
            );
        }
    }
}
