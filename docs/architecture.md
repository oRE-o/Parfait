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

## Incremental canvas rendering

Each stroke accumulates clipped, half-open `PixelRect` dab bounds. The UI drains these after each input event and unions them for the frame. `Document::composite_region` returns tightly packed RGBA for that region; the UI applies checkerboard coordinates in document space and sends `TextureHandle::set_partial`. A revision mismatch before processing canvas input forces a full refresh for external edits such as undo, layer changes or project replacement. The navigator reuses the same texture. Damage is transient and never changes the serialized document format.

## Storage protocol

Format v2 is SQLite (`user_version=2`) with `snapshots` and `session` tables. A snapshot stores the complete document using fixed-integer bincode 1 encoding, zlib compression and SHA-256 of the uncompressed bytes. Deserialization is size-limited and followed by document validation. The limit is 512 MiB uncompressed per snapshot. V2 uncompressed bytes begin with `PARFAIT\0SNAP2\0` followed by the document including each layer's `BlendMode`. Unprefixed v1 snapshots are decoded through the exact legacy struct layout and get Normal modes. Mixed v1/v2 recovery generations remain readable. Saving atomically sets `user_version=2` with the snapshot transaction; older builds reject v2 instead of misreading it.

SQLite uses WAL and `synchronous=FULL`. In one transaction, insert the new revision and prune to three newest generations. Readers examine generations newest-first and verify decompression, hash, decoding, and dimensions. Fallback advances beyond the broken generation so the next save becomes the head. A corrupt SQLite container itself may prevent fallback; the original file is preserved.

An OS file lock protects the recovery directory. `session.clean` becomes false on startup and true only after the normal final save succeeds. Worker acknowledgments contain revisions; UI saved state is compared to the current revision. The pending snapshot slot replaces older pending work, but never cancels a transaction already in progress. Failed saves retain the in-memory document and retry after a delay.

Normal app exit joins the worker after submitting the final snapshot. Manual Save writes an independent SQLite project and checkpoints its WAL. New/Open first write a rotating backup of the existing document. External database writing is unsupported.

## Module responsibilities

- `app.rs`: document lifecycle, save/load/export, background save status.
- `canvas.rs`: screen/document transforms, pointer dispatch, texture upload and view drawing.
- `rulers.rs`: bounded, document-anchored coordinate ruler layout and clipped painting, with no document or storage mutation.
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

The desktop workspace uses a compact menu/command bar, a narrow tool rail, scrollable brush/color palettes and a navigator/layer palette. Standard action icons come from the pinned Font Awesome Free asset, with tooltips and accessibility labels; see its bundled attribution. Layer thumbnails are refreshed from document pixels when the revision changes, deferred until pen-up during strokes. Rulers show screen-axis projections of document coordinates when the view is rotated.

`workspace.rs` owns typed palette placement, ordered side docks, drag/drop targets and floating in-app Areas. Docked and floating palettes share one header/body/grip renderer; drag keeps live geometry, collapse/visibility persist, and scroll offset is carried between hosts within the session. `preferences.rs` validates and persists versioned layout and brush state to a separate SQLite file using atomic row replacement; workspace preferences do not change the artwork format. Invalid or newer preferences are reported and preserved. `brushes.rs` owns the library, bounded image-tip ingestion and versioned JSON `.pbrush` interchange. `chrome.rs` owns the frameless title bar and native viewport window commands. Closing still passes through the save acknowledgment path.

Brush dynamics belong to `parfait-core`: `ResponseCurve` holds 2–16 normalized input/output points with input endpoints at 0 and 1 and strictly ordered interior inputs. Outputs may rise or fall. The core evaluates these exact coordinates before size/opacity modulation without per-sample allocation or resampling. Spacing, flow, roundness, angle and simple low-pass smoothing are independent settings. Brush settings are frozen at pointer-down. Smoothing can trail the cursor; this is not predictive or velocity-aware stabilization. The canvas keeps navigation and painting mutually exclusive for a held pointer gesture. Native pressure samples use the existing windowing backend; no native driver API is embedded in the core.

## Cross-platform boundaries

`ResponseCurve.controls` optionally stores two normalized Bezier control coordinates per anchor interval. Absence retains the automatic Hermite evaluator. Conversion initializes mathematically equivalent Bezier controls; pressure evaluation solves monotonic Bezier x using 24 bounded bisections and evaluates y with no allocation. Input loops, non-finite/out-of-range handles and count mismatches are rejected. Display uses the same evaluator. Preferences and `.pbrush` version 3 introduced explicit controls; version 4 also stores gap/spray settings and reads versions 1–3. Neither artwork snapshots nor device calibration change.

Windows, macOS and Linux are equal desktop targets, with mobile apps planned later. Keep document state, brush algorithms, editor commands and project-format migrations independent of platform APIs. Desktop and mobile should share the Rust core and exchange the same versioned project files.

OS integration belongs behind application-level interfaces for file access, data directories, input, lifecycle and rendering capabilities. The current desktop implementation uses eframe, native file dialogs and filesystem paths directly; those integrations still need separation before a mobile frontend is added. Do not assume the current UI framework or OpenGL backend is the final mobile solution.

Mobile suspension may skip normal exit callbacks. Recovery must rely on incremental committed saves, with lifecycle notifications used to request an additional save when possible. Touch and stylus input need normalized samples and gesture routing rather than simulated desktop shortcuts. See [platforms](platforms.md) for planned work and validation gates.

`pressure.rs` provides monotonic device calibration independent of brush presets. `calibration.rs` owns a draft editor and bounded test-pad data (4096 marks/samples). Only Apply updates the global curve and persists preferences. A missing calibration field in older JSON defaults to identity. Device calibration precedes the brush response. Test-pad strokes are separate from artwork and undo history. Preferences and `.pbrush` exports use version 4 and read versions 1–3; the curve deserializer converts five-output arrays to their exact original input coordinates and retains automatic evaluation for coordinate-only records. New curves store points and optional manual controls. Invalid curves are rejected. `Document` does not contain `Brush`, so artwork snapshot versions and recovery remain unchanged.

`GapMode` resolves Fixed/Wide/Normal/Narrow into a document-space interval from maximum diameter, independent of pressure and UI scale. `Stroke` retains distance phase across movement events and freezes all settings at pointer-down. `Stroke::spray` accepts elapsed seconds and current calibrated pressure from the shell, accumulating time independently of frame count. Each call consumes at most 100 ms and stamps at most 12 additional dabs; it shares normal damage tracking and has no OS timer or persistence access. `canvas.rs` requests repaint only during an active timed stroke and ends it on release, pointer/focus loss, modal interruption or navigation. `brush_stroke.rs` owns the reusable mode selector and detail-only spraying controls; it does not own brush persistence.

Dock rows group palettes horizontally and retain vertical row order. `dock_layout` is shared by rendering and drop preview; simulation occurs on a cloned workspace before committing. Header context menus switch a palette into an immediate native viewport or back to an in-app host. Native close requests hide the palette, and an optional native rectangle stores screen position and expanded size independently from its in-app rectangle. Older preference records default to separate rows and no native rectangle.

`curve_editor.rs` owns shared response graph rendering and interaction. Callers choose constraints and keep ownership of values/persistence; calibration and brush pressure use the same widget with different point policies. See the [UI system](ui-system.md) before adding another graph implementation.

## Layer blending

`parfait-core/src/blend.rs` defines 14 separable RGB blend modes and alpha-aware source-over composition. Blend functions operate on stored sRGB values. Transparent backdrop areas retain source color; source alpha is multiplied by layer opacity. Normal keeps the existing fast path. Full and damaged-region rendering share this implementation, also used by PNG export. `Layer::blend_mode` is included in snapshots, duplication and history; changing it increments the document revision for full texture invalidation.

`response_curve.rs` evaluates shape-preserving cubic Hermite responses without heap allocation. It is shared by device calibration, brush pressure and graph rendering/markers so the displayed curve matches drawing behavior. Saved node coordinates remain compatible; non-linear responses now interpolate smoothly rather than linearly.

`brush_details.rs` owns the compact brush-property palette and categorized live editor. Its preview renders a small temporary document through the core brush engine, caches one texture, and never edits artwork. `brushes.rs` retains import/export/library operations and shared preset saving. The detail editor blocks canvas gestures and global shortcuts while open; title-bar close, Close and Escape retain live edits.

`PropertyLayout` is a fixed nine-boolean UI model, independent of core brush physics and artwork serialization. Active preferences, preset records and per-family session drafts carry it. Missing layout fields default to the original four property rows, preserving older JSON preferences and `.pbrush` files. The detail eye column and palette use one property definition for labels, ranges, enablement and semantics. Spacing and pointer smoothing are exposed in separate Stroke and Correction categories. Contextual help and visibility changes have no document/history side effects.

Preference maintenance caches the last successfully saved typed `Preferences`, not its JSON payload. Before cloning or encoding, it compares every persisted field; `Workspace::same_preferences` excludes skipped interaction/layout caches. New persistent fields must be added to the comparison and its serialization-parity tests. Loaded image masks are shared between live and cached settings. Failed writes retain the previous snapshot so edits remain retryable; unsupported loaded records are never overwritten. Root maintenance runs once per second while idle and at 100 ms while saving/drawing/closing, without changing input-driven redraws or autosave durability.
