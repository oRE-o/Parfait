# Product vision

Parfait should feel like a lightweight drawing tool from the moment it opens: immediate strokes, a quiet canvas and compact, adjustable palettes. SAI-class lightness is the aspiration; this is not a claim of measured parity. The project is open source, with a drawing-first workflow and a flexible brush/material library.

## Priorities

1. Responsive strokes and predictable memory use, including on modest computers.
2. Reliable local autosave and recovery after unexpected termination. Performance work must not weaken durability silently.
3. A useful pen, eraser, layers and material workflow before a large catalog of tools.
4. First-class Windows, macOS and Linux desktop releases, then mobile apps sharing the Rust core and project format.
5. An optional community where creators can publish, discover and share brushes and other materials.

External AI processing is forbidden by the current project direction (2026-10-20). Do not upload artwork to AI services, add cloud-AI clients, or automatically download models. Improve local algorithms and interactions; any future model execution must use explicitly selected local resources.

Drawing, opening local materials and saving projects must work offline without an account. Community loading, browsing and downloads must never run in the stroke processing path. Avoid mandatory background services and a browser runtime in the desktop drawing shell.

## Performance acceptance

Measure release builds on documented hardware and representative documents. Track stroke frame time (including p95/p99 spikes), pen-to-display latency where hardware allows measurement, idle CPU, startup time, working-set memory and autosave stalls. A future baseline acceptance target is a 60 Hz drawing frame budget (16.7 ms) on a named modest reference machine; that target has not yet been verified.

Prefer work proportional to changed pixels. Bound history, pending saves, decoded materials and caches. Large brushes, many visible layers, long strokes, undo and saves during drawing all belong in performance coverage. Keep correctness comparisons against full compositing. See [performance measurements](performance.md) for current evidence and remaining bottlenecks.

## Community direction (later)

Start with portable local material packs and versioned metadata, then add optional publication, discovery, tags, previews and downloads. Materials need creator attribution, an explicit redistribution license, compatibility information and a content hash. Import must validate sizes and formats and never execute embedded scripts.

Before public uploads launch, define reporting, moderation, takedown and storage limits. Community accounts and online services should be separate from local document ownership; downloading a material must not make future offline use dependent on the service. Existing PNG/JPEG tips and `.pbrush` exchange are the first local building blocks, not an implemented community platform.

## Architecture direction

Keep document/brush logic in the UI-independent Rust core, durability in the storage crate, and platform input/windowing in the application. Next performance work should consider tiled copy-on-write buffers and measured compositing improvements. Preserve mobile lifecycle recovery and memory constraints when adding desktop features. See [platforms](platforms.md) and [roadmap](roadmap.md).
