# Architecture

The application owns UI state. The core owns image state. Storage receives immutable document snapshots and never reaches into the GUI.

## Dependency direction

```text
parfait (eframe UI)
  ├── parfait-core
  └── parfait-storage ── parfait-core
```

`parfait-core` has no GPU, filesystem, OS or UI dependency. Pixels are straight-alpha RGBA8, layers run bottom-to-top, and a document has one active layer. Every persisted state change advances a monotonic revision, including undo/redo and layer selection. Validation bounds dimensions, layer count, opacity and buffer lengths.

Layer pixel buffers use `Arc<Vec<u8>>`. History and autosave snapshots share unchanged buffers; edits use copy-on-write. History has a conservative 128 MiB logical budget and at most 30 entries. This is a prototype tradeoff, not a tiled image engine. The future tile store can replace the pixel representation behind core commands without coupling storage to egui.

Brush strokes freeze their brush settings at pointer-down. Samples are interpolated to spaced dabs; a dab applies coverage, opacity and optional pressure scaling. Erasing modifies alpha. PNG tips are masks; the brush color supplies their ink. Raster operations are independent of the view rotation and zoom.

## Storage protocol

Format v1 is SQLite (`user_version=1`) with `snapshots` and `session` tables. A snapshot stores the complete document using fixed-integer bincode 1 encoding, zlib compression and SHA-256 of the uncompressed bytes. Deserialization is size-limited and followed by document validation. The limit is 512 MiB uncompressed per snapshot.

SQLite uses WAL and `synchronous=FULL`. In one transaction, insert the new revision and prune to three newest generations. Readers examine generations newest-first and verify decompression, hash, decoding, and dimensions. Fallback advances beyond the broken generation so the next save becomes the head. A corrupt SQLite container itself may prevent fallback; the original file is preserved.

An OS file lock protects the recovery directory. `session.clean` becomes false on startup and true only after the normal final save succeeds. Worker acknowledgments contain revisions; UI saved state is compared to the current revision. The pending snapshot slot replaces older pending work, but never cancels a transaction already in progress. Failed saves retain the in-memory document and retry after a delay.

Normal app exit joins the worker after submitting the final snapshot. Manual Save writes an independent SQLite project and checkpoints its WAL. New/Open first write a rotating backup of the existing document. External database writing is unsupported.

## Module responsibilities

- `app.rs`: document lifecycle, save/load/export, background save status.
- `canvas.rs`: screen/document transforms, pointer dispatch, texture upload and view drawing.
- `panels.rs`: tools, navigator, layer controls and status.
- `shortcuts.rs`: keyboard commands and help text.
- `theme.rs`: visual style and optional system font discovery.
- `widgets.rs`: palette controls, color wheel and accessible icon buttons using the bundled Font Awesome Free font.
- `document.rs`: layer data, validation, compositing.
- `brush.rs`: sample interpolation and raster dabs.
- `history.rs`: reversible document snapshots.
- Storage `lib.rs`: on-disk format, validation, atomic saves and session locks.
- Storage `worker.rs`: queue ownership, asynchronous writes and acknowledgments.

Next changes should introduce typed editor commands before adding many tools, a tile representation before promising large canvases, and explicit schema migrations before changing persisted fields.

The desktop workspace uses a compact menu/command bar, a narrow tool rail, scrollable brush/color palettes and a navigator/layer palette. Standard action icons come from the pinned Font Awesome Free asset, with tooltips and accessibility labels; see its bundled attribution. Side palettes can be resized or hidden. This is a fixed palette layout, not a drag-and-drop docking system. Layer thumbnails are refreshed from document pixels when the revision changes. Rulers show screen-axis projections of document coordinates when the view is rotated.

## Cross-platform boundaries

Windows, macOS and Linux are equal desktop targets, with mobile apps planned later. Keep document state, brush algorithms, editor commands and project-format migrations independent of platform APIs. Desktop and mobile should share the Rust core and exchange the same versioned project files.

OS integration belongs behind application-level interfaces for file access, data directories, input, lifecycle and rendering capabilities. The current desktop implementation uses eframe, native file dialogs and filesystem paths directly; those integrations still need separation before a mobile frontend is added. Do not assume the current UI framework or OpenGL backend is the final mobile solution.

Mobile suspension may skip normal exit callbacks. Recovery must rely on incremental committed saves, with lifecycle notifications used to request an additional save when possible. Touch and stylus input need normalized samples and gesture routing rather than simulated desktop shortcuts. See [platforms](platforms.md) for planned work and validation gates.
