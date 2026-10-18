# Parfait

A lightweight, open-source drawing app in Rust. A quiet canvas, familiar pen-first controls, layers, and automatic local recovery.

**Early desktop prototype (0.1).** The first version prioritizes a complete draw → save → restart workflow over a large tool catalog.

![Parfait drawing workspace](docs/images/workspace.png)

## Product direction

Prioritize immediate strokes, low resource use and reliable recovery. Drawing stays local and offline; a future optional community will let creators share brushes and materials. See the [project vision](docs/vision.md) and [performance evidence](docs/performance.md).

## Platform direction

Windows, macOS and Linux are first-class desktop targets. Mobile apps are planned after the desktop foundation, with Android and iOS/iPadOS as intended targets. The Rust drawing core and project format should be shared across platforms; input, lifecycle, file access and UI layout must adapt to each device.

The current prototype has been built and tested on Windows only. macOS, Linux and mobile support are goals, not verified release claims. See the [platform plan](docs/platforms.md) for status and acceptance criteria.

## Try it

Install a current stable Rust toolchain and the platform build tools, then:

```sh
cargo run -p parfait
```

On Windows, the standard MSVC toolchain requires Visual Studio C++ Build Tools. The GNU toolchain also works with MinGW on PATH. The app uses native egui/eframe and OpenGL; no browser runtime is required.

```sh
cargo build --release -p parfait
cargo test --workspace
```

Run an isolated session with `parfait --data-dir PATH`. Otherwise recovery data lives in `%LOCALAPPDATA%/Parfait` on Windows, `$XDG_DATA_HOME/Parfait` or `~/.local/share/Parfait` on other desktops. Only one app can write a given recovery folder at a time.

## In this version

- Round and soft brushes, eraser, size and opacity, color picker and palette.
- Import PNG brush tips: alpha masks for transparent PNGs; inverted luminance for opaque PNGs.
- Raster layers: add, duplicate, remove, reorder, rename, show/hide, lock, opacity and 14 blend modes.
- Bounded undo/redo with copy-on-write pixel snapshots.
- Pan, cursor-centered zoom, view rotation, fit view and navigator preview.
- Local `.parfait` projects, PNG export, background autosave and startup recovery.
- Dark workspace: tools on the left, canvas in the middle, navigator and layers on the right.
- Compact desktop palettes: icon tool rail, brush previews and size presets, hue/SV color wheel, canvas rulers and live layer thumbnails.
- Draggable palettes: reorder or move between side docks, dock side by side, detach into in-app palettes or independent desktop windows, resize and restore your layout on restart.
- Ten built-in brush presets including hard/soft erasers, grouped local brush library, search and persistent favorites, batch PNG/JPEG tip import and portable `.pbrush` preset import/export.
- Editable pressure curve, pressure-driven size/opacity, minimum size, flow, spacing, hardness, tip angle/roundness and smoothing.
- Application-owned title bar with window dragging, maximize/restore, minimize, resize edges and save-aware close.

## Workspace and brush settings

Drag a palette's **≡ header** to move it: the actual palette follows the pointer with its current width and body height. Docked and floating palettes use the same compact header and content; floating palettes have no extra window title bar. Drop near the left/right quarter of a docked palette to place it beside that palette in the same row; the center upper/lower half inserts a row above/below. The filled blue rectangle previews the resulting footprint. Drop at the left/right app edge to append to that dock. Press **Escape** during a move to restore its original placement.

The small **− / +** button collapses or expands the body without losing its height; **×** hides the palette. Reopen it from **View → Palettes**. These controls work in both docked and floating states, and visibility/collapse state persist on restart. Floating palettes remain inside the app window. To create an independent desktop window, right-click a palette header and choose **Open in separate window**. Drag its header to move the OS window; use **Dock on left/right** or **Float inside main window** in the same menu to return it. Closing that window hides only its palette, which can be reopened from **View → Palettes**. Native window position/size are saved separately from in-app floating position. Windows creation and close were checked; macOS/Linux native behavior still needs validation. Drag the bottom grip vertically to resize a docked body; in a floating palette or separate window the grip resizes width and height. Rows share width evenly; a dock expands to fit side-by-side panels up to its width limit, then scrolls horizontally. Drag a dock boundary to resize the dock. **View → Reset palette layout** restores the defaults. See the [UI system](docs/ui-system.md).

Layout, active brush settings and the local preset library are saved to `preferences.sqlite` in the recovery directory approximately once per second while not drawing, and on normal exit. Artwork remains in its separate recovery database. Settings changed immediately before a forced termination may not yet be saved.

Response graphs share a compact square editor with grid lines, percentage labels and square control points.

Brush graphs also expose **round tangent handles** connected to the selected square anchor. Drag a round handle to change curvature and its influence length without moving the anchor or adding a point. The two sides are independent. Handle X/Y fields and focused arrow keys edit the handle, not its anchor. **Auto curvature** discards manual handles while retaining anchor coordinates. Merely selecting a handle does not change an automatic curve. Device calibration retains its constrained automatic interpolation.

**Details → Stroke** now separates Gap modes from the editable spacing value. **Fixed** uses your 1–1000% value (12% by default); **Wide / Normal / Narrow** use 50% / 25% / 10% of maximum brush diameter. These are Parfait's own ratios, with a 0.5 document-pixel minimum, not a promise of another engine's exact output. Other modes disable but retain your Fixed value. The same selector appears when Spacing is pinned in Tool property. Narrow gaps require more dabs and more work.

Enable **Continuous spraying** to keep building paint while holding still. **Spray rate** sets 1–120 additional dabs per second (default 30); normal distance-based dabs remain active while moving. Flow still controls deposition per dab. Timed dabs use elapsed time, not frame count; a single update accepts at most 100 ms to prevent a burst after a long pause. Release, focus loss, pointer loss, navigation or opening a dialog ends the stroke. The static preview shows spacing but not stationary buildup. New **Dotted pen** and **Continuous airbrush** presets provide starting points; use **Manage → Restore missing built-ins** in an existing library. Ribbon, multiple-tip repeat order and velocity/pressure-driven spacing are not implemented. Launch with `--stroke-settings` to inspect this category directly.

In **Tool property**, click the narrow dynamics button beside **Brush size** or **Opacity**. The pressure-arrow icon means pressure is enabled; a slashed box means it is off. Click inside the graph to add a point (up to 16 total), drag interior points in both axes, or type input/output percentages. Endpoints keep input at 0% and 100% but their output can change. Delete an interior point with Delete/Backspace, the button, or by releasing it more than 12 logical pixels outside the graph. Arrow keys move 1%; Shift moves 10%. Rising and falling output segments are supported. Linear resets to two endpoints; Soft and Firm provide starting shapes. Size and opacity use the same response curve. Windows uses the backend's Windows Ink/pointer pressure path; tablet drivers need to deliver those events (legacy Wintab-only mode is not implemented). Mouse input uses full pressure. Physical tablets and Linux/macOS pen backends still require hardware verification.

Use **Tool property → Details… → Pressure → Calibrate tablet…** to open the device calibration window. Draw light and firm strokes in the test pad, then choose **Suggest from strokes**. The suggestion maps the measured 95th percentile to full output and requires at least 20 varied pressure samples; mouse input only previews and cannot generate a suggestion. Click the graph to add a point, drag it in both axes, or delete the selected interior point (up to 16 points, monotonic response, fixed endpoints). Preview strokes update immediately with the draft curve. **Apply & close** saves the calibration; **Cancel / close**, Escape or the title-bar × discards it. Action buttons stay above the scrollable content, including at the minimum window size; the main window controls remain available. **Reset curve** restores an identity draft. Curves smoothly pass through their control points, using the same interpolation for display and actual pressure. This global device curve runs before individual brush curves and does not modify brush preset files. You can also open the window on startup with `--calibrate-pressure`.

Use **Sub tools → Manage → Import tips / presets** for multiple PNG/JPEG tips or `.pbrush` files. The Ink, Sketch, Paint, Airbrush, Eraser and Materials filters organize implemented tips; All shows the whole library. Search matches names, and the star filter shows favorites. Filters combine; **Show all brushes** clears an empty result. Row stars persist locally. Tool-rail buttons select a matching brush and remember each family's last unsaved settings for the current session. Picking a library row loads its saved settings, preserving the current color; eraser presets activate erasing and painting presets exit erasing. List previews use the actual brush engine in neutral ink, with bounded caching deferred while drawing.

**Tool property → Save** updates the named preset; **Restore** reloads its saved settings while retaining the current color. An asterisk marks unsaved preset edits. **Manage** also offers renaming/saving as another preset, export, and restoring missing built-ins. Export includes tip data and settings. Imported tips are limited to 256 × 256, individual files to 16 MiB and the library to 64 presets. Older settings and `.pbrush` files remain readable. Existing workspace layouts are retained; reset the palette layout to try the new defaults, which start with brush-size presets collapsed. Proprietary brush formats and exact cross-engine brush matching are not implemented.

Preferences and `.pbrush` exports use version 4 for gap modes and timed spraying, retaining optional explicit Bezier handles introduced in version 3. Versions 1–3 remain readable: older brushes default to Fixed gap and no timed spraying; five-output arrays migrate to their original input positions, and coordinate-only curves retain automatic interpolation. All settings survive save/restore and restart. Older app builds cannot read the new settings; artwork snapshots and crash-recovery formats are unchanged.

Ctrl/Cmd + Plus/Minus no longer scales the UI. Space + pointer drag pans; pressing Space during a stroke ends drawing before navigation and drawing resumes only on a new pointer-down.

## Controls

| Action | Shortcut |
| --- | --- |
| Pen / pencil / paint / airbrush / eraser | P / K / B / A / E |
| Hand / eyedropper | H / I |
| Brush size | [ / ] |
| Move canvas | Space + drag, or middle-button drag |
| Rotate view | Shift + Space + drag |
| Zoom view | Ctrl + `+` / `-` (`=` also zooms in); wheel or Ctrl + wheel at cursor; Ctrl + Space + drag |
| Pick canvas color | Alt + click |
| Fit and reset rotation | Ctrl + 0 |
| Undo / redo | Ctrl + Z / Ctrl + Y or Ctrl + Shift + Z |
| Add layer | Ctrl + Shift + N |
| New / open | Ctrl + N / Ctrl + O |
| Save / save as | Ctrl + S / Ctrl + Shift + S |
| Export PNG | Ctrl + Shift + E |
| Hide side panels / help | Tab / F1 |

Unused Ctrl/Cmd combinations keep ordinary key actions; registered combinations take precedence. Middle-button dragging pans even with Ctrl held. Keyboard shortcuts pause while editing a text field. View transformations do not alter image pixels.

## Recovery contract

An unnamed drawing is protected from the first edit. Completed strokes request an immediate background save; longer edits request a snapshot approximately every 400 ms. Pending work is coalesced so slow disks cannot create an unlimited queue.

The UI shows **Saved locally** only after the worker confirms a committed SQLite transaction. Three checksum-verified compressed snapshots are retained. An invalid newest payload falls back to an earlier valid generation, and the app reports that fallback. An unrecoverable database produces an error rather than silently starting over.

An unexpected exit restores the latest valid committed snapshot on next launch. Input not yet committed may be lost. Disk failure and real power-loss durability are not guaranteed by the process-kill tests. Keep exported project backups for important work.

Before New/Open replaces the current drawing, the app creates `before-new.parfait` / `before-open.parfait` in its recovery directory. Those backups are single rotating slots. Save a named project to keep an independent copy. Undo history is session-local; pixel and layer state survives restart.

## Development

See [architecture](docs/architecture.md), [testing](docs/testing.md), and [roadmap](docs/roadmap.md).

```text
apps/parfait/             Native application, panels, input and canvas view
crates/parfait-core/      Raster document, brush engine, undo/redo
crates/parfait-storage/   Versioned SQLite snapshots, worker, recovery tests
docs/                    Architecture, testing, roadmap
```

## Current limits

The prototype starts at 1024 × 768 and uses whole-layer CPU buffers with changed-region CPU compositing and partial texture updates during strokes. Large-canvas tiled rendering is planned. Physical pen pressure/tilt and device-specific behavior still need hardware validation. Cross-window drag docking, nested split layouts, ABR/SUT import, advanced mixing, selection, transforms, and liquify are not implemented. Linux/macOS source portability has not yet been validated on hardware. See the [feature plan](docs/feature-plan.md) for remaining editing tools and UI groups.

## License

MIT. See [LICENSE](LICENSE). Contributions should include source provenance, tests for behavior changes, and no third-party assets without redistribution permission.

Toolbar icons use bundled Font Awesome Free 6.7.2 (SIL OFL 1.1 for the font). See [asset attribution](apps/parfait/assets/fontawesome/README.md) and Help → Third-party notices in the app.

### Layer blend modes

Choose the active layer's mode in the Layers palette, beside opacity: Normal, Multiply, Screen, Overlay, Darken, Lighten, Color dodge, Color burn, Hard light, Soft light, Difference, Exclusion, Add and Subtract. Modes affect the canvas, navigator and PNG export; changes support undo/redo and persist in projects and recovery snapshots.

Older projects open with Normal layers. Saving writes format v2, which older Parfait builds cannot open. Blending currently uses stored sRGB channel values, not linear-light or color-managed compositing.

### Compact brush controls

The Tool property palette starts with size, opacity, flow and smoothing. Rows use thin filled bars, unframed editable numbers, stacked increment/decrement chevrons and horizontal dividers. Brush size uses a logarithmic track; numbers remain in document pixels. Other units are shown in tooltips. **Details…** opens six categories: Brush size, Ink, Brush tip, Stroke, Correction and Pressure. Each numeric parameter has an eye button to show or hide it in Tool property. Choose from nine implemented parameters, including minimum size, hardness, roundness, angle and spacing. **Save preset** stores both values and property visibility; **Restore saved** restores both while retaining the current color. **Default property layout** resets only visibility. Old preferences and `.pbrush` files default to the original four rows. The active layout is also saved in local preferences without requiring a preset save.

The detail window uses a fixed vertical category list, the same parameter rows as the palette, a shared live preview, and a parameter guide that follows the hovered control. Smoothing belongs to Correction; spacing belongs to Stroke. Unsupported controls are disabled with explanations. Hand and Eyedropper show their own instructions. Changes apply immediately; Close, title-bar × and Escape retain edits. Escape dismisses a dynamics child before its parent detail window. Device calibration retains its separate Apply/Cancel behavior. Preview size is capped at 38 px in neutral ink and does not simulate erasing, device calibration or pointer smoothing. `--brush-details` opens the editor and `--brush-dynamics` opens size dynamics for UI inspection.
