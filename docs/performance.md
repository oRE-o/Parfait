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

Whole-layer copy-on-write buffers can still cause a full layer copy after history checkpoints or autosave snapshots. Full-document serialization/compression happens on the background worker but consumes CPU/memory bandwidth. Settings persistence after pen-up can still be expensive when a large material library actually changes. Unchanged preferences now skip cloning/serialization through a typed saved-state comparison, described below. Tiled snapshots, asynchronous changed-preference writes and real-device p95/p99 measurements remain follow-up work; current measurements do not establish the product's overall lightweight target.

## Blend-mode workload

Run `cargo run --release -p parfait-core --example blend_bench` for 30 compositions of a 1024 × 768 document with eight populated translucent layers (pixel alpha 180/255, layer opacity 0.8). It compares the full canvas with a 32 × 32 damage rectangle. Allocation and CPU blending are included; strokes, UI, GPU, history and autosave are excluded. This is deliberately heavier than the mostly blank-layer stroke benchmark above.

One Windows development-machine release run (same CPU/toolchain as above, 2026-10-18):

| Mode | Full canvas, ms/update | 32 × 32 region, ms/update |
| --- | ---: | ---: |
| Normal | 170.661 | 0.188 |
| Multiply | 211.501 | 0.444 |
| Soft light | 329.872 | 0.324 |

These single-run averages are sensitive to scheduling and are not pen latency measurements. Complex modes cost more, and full refreshes on heavily populated documents remain a performance limitation. Painting continues to use damage regions.

## Smooth response follow-up

After shared cubic pressure interpolation and blend-mode integration, the same `stroke_bench` command on the Windows development machine above produced these single-run averages (2026-10-18):

| Canvas / layers | Full composition, ms/update | Partial composition, ms/update |
| --- | ---: | ---: |
| 1024 × 768 / 2 | 9.604 | 0.019 |
| 2048 × 2048 / 8 | 177.567 | 0.032 |

This measures the combined rendering path at that revision, not isolated spline overhead or a controlled before/after comparison. Full composition remains expensive and deserves separate profiling; stroke damage updates remain local. Response evaluation does not allocate per sample. GUI, GPU, autosave and device latency remain excluded.

## Variable response point workload

The brush now owns 2–16 response coordinates and samples them without creating temporary points. Run `cargo run --release -p parfait-core --example stroke_bench`, then the same command with `-- --dense-curve` for 16 non-uniform input points and a square-root output response. One macOS arm64 Apple M1 run using Rust 1.99.1 on 2026-10-19 produced these single-run averages:

| Points | Canvas / layers | Full, ms/update | Partial, ms/update | Partial bytes/update |
| --- | --- | ---: | ---: | ---: |
| 2 | 1024 × 768 / 2 | 4.034 | 0.008 | 1,769 |
| 2 | 2048 × 2048 / 8 | 76.905 | 0.013 | 1,769 |
| 16 | 1024 × 768 / 2 | 3.657 | 0.009 | 2,198 |
| 16 | 2048 × 2048 / 8 | 73.228 | 0.017 | 2,198 |

The dense curve changes painted diameter and damaged-region size, so these are representative workloads, not an isolated point-count overhead measurement. Different full-path timings are not evidence that more points improve performance. GUI, GPU, history, saving and physical input latency remain excluded. Pixel-equivalence tests also exercise arbitrary-input falling curves.

For explicit Bezier evaluation, add `--bezier` (for example `-- --dense-curve --bezier`). It initializes equivalent manual controls on the same response, using a bounded 24-step input solve per sample without heap allocation. A single Apple M1/macOS arm64/Rust 1.99.1 release run on 2026-10-19 with 16 points produced 3.874/0.010 ms per full/partial update at 1024 × 768 with 2 layers and 71.522/0.017 ms at 2048 × 2048 with 8 layers; partial upload payload was 2,198 bytes/update. These whole-workload averages do not isolate curve-evaluation cost or establish pen latency or a speedup. Region/full pixel-equivalence tests include manual handles.

## Gap and timed-spray workload

Run `cargo run --release -p parfait-core --example stroke_bench -- --gap-narrow --spray` to use the Narrow interval and add 1/60 s of stationary accumulation at the current position on each movement update. This uses a 24 px round brush, identity pressure curve and 30 extra dabs/second in the same 240-update workload. The time function performs no allocation or filesystem access, caps one elapsed update at 100 ms and emits at most 12 additional dabs. This bounds catch-up work, not total brush cost: 512 px tips at high rates and frequent copy-on-write snapshots remain expensive.

An Apple M1/macOS arm64/Rust 1.99.1 release run on 2026-10-19 produced these single-run averages:

| Canvas / layers | Full, ms/update | Partial, ms/update | Partial bytes/update |
| --- | ---: | ---: | ---: |
| 1024 × 768 / 2 | 3.739 | 0.014 | 2,703 |
| 2048 × 2048 / 8 | 71.016 | 0.024 | 2,703 |

This includes CPU dabs/compositing but excludes GUI, GPU upload, saving, input delivery and actual frame scheduling. It is not a measured stationary pen latency, application frame budget or a cross-product comparison. Partial/full pixel equivalence is separately tested with timed dabs across every tip and erasing mode.

## Lightweight maintenance audit

Image-tip masks and document pixels already use shared `Arc` buffers. No new runtime dependencies or removal of accessibility, recovery, validation or image formats were needed for this audit. Build caches are not distribution size or runtime memory.

Unchanged settings previously cloned and serialized the complete preference record every maintenance interval, then compared two JSON strings. The application now compares typed persisted fields first and returns before cloning, JSON encoding or database access. The last successfully saved typed snapshot shares image masks with the live library; the complete saved JSON string is no longer kept resident. Runtime-only workspace/brush-library caches are excluded. Validation and the existing synchronous-FULL database write still run on actual changes, and only successful writes advance the cache. Older records still upgrade to version 4; failed/future-version loads remain protected.

Reproduce the opt-in, non-shipping benchmark:

```sh
cargo test --release -p parfait preferences::tests::unchanged_material_library_poll_benchmark -- --ignored --exact --nocapture --test-threads=1
```

The fixture has 64 presets sharing a 256 × 256 image mask, a 16,810,960-byte JSON record, an initial save outside the timer and 20 forced unchanged-maintenance calls. Three separate runs per executable on Apple M1/macOS 15.7.5/arm64/Rust 1.99.1 with thin LTO on 2026-10-19 gave:

| Workload | Before, ms/call | After, ms/call |
| --- | ---: | ---: |
| Unchanged material-library maintenance | 22.322–25.089 | 0.000981–0.002252 |

These measure only the unchanged-state fast path, not changed-library saving, normal per-frame cost, GPU work, physical pen latency or total application memory. A new typed snapshot has a small bounded metadata cost; image payloads remain shared. OS memory-accounting samples are not evidence of an equivalent net memory reduction or parity with another painting application.

Idle root-window maintenance requests changed from 100 ms to 1 second. Pending saves, an active stroke and pending close retain 100 ms maintenance, and continuous spraying retains its independent shorter interval. Native input and framework animations can request frames immediately. Artwork autosave's 400 ms submission interval, acknowledgments, final flush and recovery durability are unchanged. This reduces scheduled idle polling, not a measured 90% reduction in CPU or power.

Keep release packaging, idle CPU/working-set measurements, populated-layer workloads, large-library edits and save/undo spikes in future performance gates. Do not trade away safe saves or add unbounded caches to improve a small benchmark.
