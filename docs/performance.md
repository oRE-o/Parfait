# Stroke performance

## Changed path

Previously, every painted frame composited all pixels of every visible layer, applied a checkerboard to the entire canvas and uploaded a complete texture. The canvas now accumulates dab bounds for each input batch, composites only that rectangle and submits a partial texture update. Undo, layer edits and opening another document still request a full refresh. Bounds are transient; the project format is unchanged.

Opaque pixels take a fast blend path. Picking a color composites one pixel. Layer thumbnails catch up after the stroke; the canvas and navigator stay live. Preferences serialization/writes are deferred during an active stroke. Artwork autosave still requests a background snapshot every 400 ms and immediately after a completed stroke.

## Reproduce

```sh
cargo run --release -p parfait-core --example stroke_bench
cargo test --workspace
```

The benchmark performs 240 identical incremental updates with a 24 px pressure-varying round brush, comparing full and damaged-region compositing in the same executable. It paints one layer; other layers are blank except for opaque paper. Initial allocation/full rendering is outside the timer. It includes dabs, allocation and CPU layer composition; it **excludes** GUI/checkerboard conversion, GPU upload, input hardware, history snapshots and autosave. It does not measure end-to-end application latency.

One Windows x86-64 GNU release run on an Intel Core i7-8700 @ 3.20 GHz development machine (Rust 1.99.1, thin LTO, 2026-10-18):

| Canvas / layers | Full composition, ms/update | Partial composition, ms/update | Full / partial RGBA bytes per update |
| --- | ---: | ---: | ---: |
| 1024 × 768 / 2 | 3.486 | 0.017 | 3,145,728 / 1,769 |
| 2048 × 2048 / 8 | 51.207 | 0.020 | 16,777,216 / 1,769 |

These are workload-specific averages, not p95 latency, a fixed speedup for all drawings, or a comparison against another application. The full path here includes the new opaque-pixel fast path, so this is a path comparison rather than a checkout-to-checkout benchmark. Wide strokes or input spanning a large rectangle reduce the benefit.

## Correctness and remaining costs

Tests compare incremental pixels to full composition for every built-in tip, image tips, erasing, pressure, layer opacity and clipped/outside strokes. A headless application test reconstructs the texture from actual full/partial upload commands, checks checkerboard alignment and verifies full invalidation after a layer edit and undo. Existing forced-process-exit recovery tests remain required.

Whole-layer copy-on-write buffers can still cause a full layer copy after history checkpoints or autosave snapshots. Full-document serialization/compression happens on the background worker but consumes CPU/memory bandwidth. Settings persistence after pen-up can still be expensive with large material libraries. Tiled snapshots, dirty preference tracking and real-device p95/p99 measurements remain follow-up work; current measurements do not establish the product's overall lightweight target.

## Blend-mode workload

Run `cargo run --release -p parfait-core --example blend_bench` for 30 compositions of a 1024 × 768 document with eight populated translucent layers (pixel alpha 180/255, layer opacity 0.8). It compares the full canvas with a 32 × 32 damage rectangle. Allocation and CPU blending are included; strokes, UI, GPU, history and autosave are excluded. This is deliberately heavier than the mostly blank-layer stroke benchmark above.

One Windows development-machine release run (same CPU/toolchain as above, 2026-10-18):

| Mode | Full canvas, ms/update | 32 × 32 region, ms/update |
| --- | ---: | ---: |
| Normal | 170.661 | 0.188 |
| Multiply | 211.501 | 0.444 |
| Soft light | 329.872 | 0.324 |

These single-run averages are sensitive to scheduling and are not pen latency measurements. Complex modes cost more, and full refreshes on heavily populated documents remain a performance limitation. Painting continues to use damage regions.
