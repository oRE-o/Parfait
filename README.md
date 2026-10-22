# Parfait

A lightweight, open-source drawing app in Rust. A quiet canvas, familiar pen-first controls, layers, and automatic local recovery.

**Early desktop prototype (0.1).** The first version prioritizes a complete draw → save → restart workflow over a large tool catalog.

개발 계획은 [개발 문서](docs/README.md)에서 읽을 수 있습니다. [로드맵](docs/roadmap.md)과 [구현 현황](docs/status.md)에 개발 순서와 현재 범위를 정리했습니다.

![Parfait drawing workspace](docs/images/workspace.png)

## Product direction

Prioritize immediate strokes, low resource use and reliable recovery. Drawing stays local and offline; a future optional community will let creators share brushes and materials. See the [project vision](docs/vision.md) and [performance evidence](docs/performance.md).

## Platform direction

Windows, macOS and Linux are first-class desktop targets. Mobile apps are planned after the desktop foundation, with Android and iOS/iPadOS as intended targets. The Rust drawing core and project format should be shared across platforms; input, lifecycle, file access and UI layout must adapt to each device.

The prototype has Windows development coverage and an arm64 macOS build with isolated native UI/restart checks. Linux and mobile remain unverified; physical tablet and multi-monitor behavior still need hardware coverage. See the [platform plan](docs/platforms.md) for status and acceptance criteria.

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
- Raster layers: add, duplicate, remove, reorder, rename, show/hide, full/alpha lock, stable identities, opacity and 14 blend modes.
- Named, bounded undo/redo with copy-on-write snapshots and an optional History palette.
- Pan, cursor-centered zoom, view rotation, fit view and navigator preview.
- Local `.parfait` projects, PNG/JPEG import as a new document or layer, PNG export, background autosave and startup recovery.
- Dark workspace: tools on the left, canvas in the middle, navigator and layers on the right.
- Compact desktop palettes: icon tool rail, brush previews and size presets, hue/SV color wheel, canvas rulers and live layer thumbnails.
- Draggable palettes: reorder or move between side docks, dock side by side, detach into in-app palettes or independent desktop windows, resize and restore your layout on restart.
- Fourteen built-in brush presets including a capped marker, hard/soft erasers, color blend, local blur and spatial smudge, grouped local brush library, search and persistent favorites, batch PNG/JPEG tip import and portable `.pbrush` preset import/export.
- Editable pressure curve, pressure-driven size/opacity, minimum size, brush density, optional per-stroke opacity limits, spacing, hardness, tip angle/roundness and smoothing.
- Application-owned title bar with window dragging, maximize/restore, minimize, resize edges and save-aware close.

## Workspace and brush settings

Drag a palette's **≡ header** to move it: the actual palette follows the pointer with its current width and body height. Docked and floating palettes use the same compact header and content; floating palettes have no extra window title bar. Drop near the left/right quarter of a docked palette to place it beside that palette in the same row; the center upper/lower half inserts a row above/below. The filled blue rectangle previews the resulting footprint. Drop at the left/right app edge to append to that dock. Press **Escape** during a move to restore its original placement.

The small **− / +** button collapses or expands the body without losing its height; **×** hides the palette. Reopen it from **창**. These controls work in both docked and floating states, and visibility/collapse state persist on restart. Floating palettes remain inside the app window. To create an independent desktop window, right-click a palette header and choose **Open in separate window**. Drag its header to move the OS window; use **Dock on left/right** or **Float inside main window** in the same menu to return it. Closing that window hides only its palette, which can be reopened from **창**. Native window position/size are saved separately from in-app floating position. Windows creation and close were checked; macOS/Linux native behavior still needs validation. Drag the full boundary between stacked palettes to share their heights, or the vertical boundary between side-by-side cells to adjust their width ratio. Collapsed palettes retain their sizes, and width ratios survive restart. Floating palettes resize from their edges/corners; independent windows use OS edge resizing. A dock expands to fit side-by-side panels up to its width limit, then scrolls horizontally. Drag the canvas-side dock boundary to resize the whole dock. **창 → 팔레트 배치 초기화** restores the defaults. See the [UI system](docs/ui-system.md).

Layout, active brush settings and the local preset library are saved to `preferences.sqlite` in the recovery directory approximately once per second while not drawing, and on normal exit. Artwork remains in its separate recovery database. Settings changed immediately before a forced termination may not yet be saved.

Response graphs share a compact square editor with grid lines, percentage labels and square control points.

Choose **Blend** on the rail (J) for **Color blend**, **Local blur** and **Smudge**. All three use only the active, visible, unlocked raster layer, not the drawing color, paper or merged layers. Color blend carries a local average; Smudge carries a spatial pixel patch, retaining the starting pattern instead of averaging it away. **Color stretch** sets previous-color/pattern retention. Smudge at 0% or while stationary has no effect; its spray controls are disabled without discarding saved values. Pressure-driven diameter changes resample the patch, which can scale its texture. Dragging transparent pixels can reduce the active layer's alpha. Local blur uses an alpha-safe box filter with automatic or fixed radius (1–32 document px). Size, opacity, brush density, tips, spacing and pressure use the shared brush pipeline. **Details → Ink → Operation** selects the operation; eyes pin additional parameters. Previews process real seed pixels, including stripes for Smudge. Missing blend presets are added on first selecting Blend when library space permits. Pigment-adding wet mixing, clone stamping and merged-layer sampling remain unimplemented.

For Paint and Erase, **Details → Ink → Accumulation** chooses **Per dab (build up)**, the unchanged default, or **Per stroke (capped)**. Capped mode limits the contribution of overlapping dabs within one pointer-down gesture to **Opacity**; lifting and starting another stroke builds up again. **Brush tip → Brush density** controls how quickly each dab approaches that limit (the saved `flow` field is unchanged). Opacity pressure sets a local target without removing stronger marks already deposited. Blend/Blur/Smudge keep per-dab strength and disable this selector without discarding its saved value. Try **Capped marker** under Paint; existing libraries can add it with **Manage → Restore missing built-ins**. Capped strokes allocate contribution tiles only where needed, up to 64 MiB plus 32 KiB indexing for a fully covered 4096² canvas, in addition to existing layer/history costs. This is not a tiled document engine.

Brush graphs also expose **round tangent handles** connected to the selected square anchor. Drag a round handle to change curvature and its influence length without moving the anchor or adding a point. The two sides are independent. Handle X/Y fields and focused arrow keys edit the handle, not its anchor. **Auto curvature** discards manual handles while retaining anchor coordinates. Merely selecting a handle does not change an automatic curve. Device calibration retains its constrained automatic interpolation.

**Details → Stroke** now separates Gap modes from the editable spacing value. **Fixed** uses your 1–1000% value (12% by default); **Wide / Normal / Narrow** use 50% / 25% / 10% of maximum brush diameter. These are Parfait's own ratios, with a 0.5 document-pixel minimum, not a promise of another engine's exact output. Other modes disable but retain your Fixed value. The same selector appears when Spacing is pinned in Tool property. Narrow gaps require more dabs and more work.

For **Fixed** Gap, **Spacing dynamics** enables an independent pressure curve and **Minimum gap** (0–100%, default 10%). Each dab's pressure sets the following interval, with a 0.5 px floor. Linear increases the interval with pressure; Reverse reduces it. Size/opacity responses are unaffected. Automatic Gap modes disable but retain these values. Active settings and explicitly saved presets survive restart and `.pbrush` exchange.

Enable **Continuous spraying** to keep building paint while holding still. **Spray rate** sets 1–120 additional dabs per second (default 30); normal distance-based dabs remain active while moving. Flow still controls deposition per dab. Timed dabs use elapsed time, not frame count; a single update accepts at most 100 ms to prevent a burst after a long pause. Release, focus loss, pointer loss, navigation or opening a dialog ends the stroke. The static preview shows spacing but not stationary buildup. New **Dotted pen** and **Continuous airbrush** presets provide starting points; use **Manage → Restore missing built-ins** in an existing library. Ribbon, multiple-tip repeat order and velocity-driven spacing are not implemented. Launch with `--stroke-settings` to inspect this category directly.

In **Tool property**, click the narrow dynamics button beside **Brush size** or **Opacity**. The pressure-arrow icon means pressure is enabled; a slashed box means it is off. Click inside the graph to add a point (up to 16 total), drag interior points in both axes, or type input/output percentages. Endpoints keep input at 0% and 100% but their output can change. Delete an interior point with Delete/Backspace, the button, or by releasing it more than 12 logical pixels outside the graph. Arrow keys move 1%; Shift moves 10%. Rising and falling output segments are supported. Linear resets to two endpoints; Soft and Firm provide starting shapes. Size and opacity use the same response curve. Windows uses the backend's Windows Ink/pointer pressure path; tablet drivers need to deliver those events (legacy Wintab-only mode is not implemented). Mouse input uses full pressure. Physical tablets and Linux/macOS pen backends still require hardware verification.

Use the wrench at the bottom of **Tool property → Sub tool detail → Pressure → Calibrate tablet…** to open the device calibration window. Draw light and firm strokes in the test pad, then choose **Suggest from strokes**. The suggestion maps the measured 95th percentile to full output and requires at least 20 varied pressure samples; mouse input only previews and cannot generate a suggestion. Click the graph to add a point, drag it in both axes, or delete the selected interior point (up to 16 points, monotonic response, fixed endpoints). Preview strokes update immediately with the draft curve. **Apply & close** saves the calibration; **Cancel / close**, Escape or the title-bar × discards it. Action buttons stay below and outside the scrollable content, including at the minimum window size; the main window controls remain available. **Reset curve** restores an identity draft. Curves smoothly pass through their control points, using the same interpolation for display and actual pressure. This global device curve runs before individual brush curves and does not modify brush preset files. You can also open the window on startup with `--calibrate-pressure`.

Use **Sub tools → Manage → Import tips / presets** for multiple PNG/JPEG tips or `.pbrush` files. The Ink, Sketch, Paint, Airbrush, Eraser and Materials filters organize implemented tips; All shows the whole library. Search matches names, and the star filter shows favorites. Filters combine; **Show all brushes** clears an empty result. Row stars persist locally. Tool-rail buttons select a matching brush and remember each family's last unsaved settings for the current session. Picking a library row loads its saved settings, preserving the current color; eraser presets activate erasing and painting presets exit erasing. List previews use the actual brush engine in neutral ink, with bounded caching deferred while drawing.

**Tool property → Save** updates the named preset; **Restore** reloads its saved settings while retaining the current color. An asterisk marks unsaved preset edits. **Manage** also offers renaming/saving as another preset, export, and restoring missing built-ins. Export includes tip data and settings. Imported tips are limited to 256 × 256, individual files to 16 MiB and the library to 64 presets. Older settings and `.pbrush` files remain readable. Existing workspace layouts are retained; reset the palette layout to try the new defaults, which start with brush-size presets collapsed. Proprietary brush formats and exact cross-engine brush matching are not implemented.

Preferences use version 13 and `.pbrush` exports version 8, reading versions 1–12 and 1–7 respectively. Older six-palette layouts retain their geometry and key bindings, with History added hidden. They retain editable shortcuts, independent pressure spacing, spatial Smudge, raster operations, gap/spray and optional Bezier handles. Missing operations default to Paint or the legacy erase flag; older brushes default to Fixed gap, no timed spraying, disabled pressure spacing and per-dab accumulation. Five-output arrays migrate to their original input positions; coordinate-only curves retain automatic interpolation. Saved settings survive restart. Older builds cannot read newer settings; artwork now saves v3 with exact v1/v2 readers.

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
| Select top painted layer | Ctrl/Cmd + Shift + click |
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

Use **파일 → 가져오기 → 이미지를 새 그림으로… / 이미지를 레이어로…** for local PNG/JPEG artwork. Imports are decoded on one bounded background job, limited to 32 MiB per file and 4096 pixels per side. New documents retain image size and alpha without binding the source image as a writable project. Layer imports are centered at original size, are undoable, and reject images larger than the canvas instead of cropping them. JPEG orientation metadata is applied; imported pixels use 8-bit RGBA without ICC color-profile conversion. Cancelled or failed imports preserve the existing artwork and source file.

An unexpected exit restores the latest valid committed snapshot on next launch. Input not yet committed may be lost. Disk failure and real power-loss durability are not guaranteed by the process-kill tests. Keep exported project backups for important work.

Before New/Open replaces the current drawing, the app creates `before-new.parfait` / `before-open.parfait` in its recovery directory. Those backups are single rotating slots. Save a named project to keep an independent copy. Undo history is session-local; pixel and layer state survives restart.

## Layer edits and History

The top menu bar is organized as **파일 · 편집 · 레이어 · 선택 · 표시 · 필터 · 창 · 도움말**. Canvas viewing controls are under **표시**; palettes and workspace controls are under **창**; general settings are grouped under **편집 → 설정**. Existing actions retain their Korean labels, shortcut hints and shared behavior. The expanded outline has 394 leaf routes and 41 submenus: 69 routes reach working actions and 325 planned/menu-unconnected entries are disabled with **미구현** and explanatory tooltips. Long menus scroll within the screen. Selection, filters, transforms, new layer types and other planned entries are not working engines merely because their names appear. Animated/time-based features are excluded.

**레이어 → 정렬** moves the active layer forward/backward or to either end without moving other checked layers. Palette Up/Down and group dragging still move the selected set. **편집 대상으로 설정** cycles through hidden and locked layers too, clearing additional selection without adding History. New keymaps use Alt+] / Alt+[; saved keymaps retain their previous assignments. **레이어 설정 → 레이어 이름 변경** opens a standalone name dialog even when palettes are hidden. Enter confirms, Escape/Cancel discards, and blank/control-character names are rejected. The dialog supports up to 128 Unicode characters and does not confirm during IME composition.

Open **창 → 작업 내역** for named chronological edits. The highlighted row is the current state; gray future rows remain clickable after Undo. Clicking a row restores the state after that edit. The palette, menu and keyboard use the same Undo/Redo cursor. A real new edit replaces future rows; unchanged clicks, zero-effect strokes, refused layer creation and layer selection do not. History retains up to 30 edits plus the earliest retained state, reduced by a conservative 128 MiB logical history budget with a minimum of one edit. It is not saved with the drawing.

Layer opacity groups a complete slider drag into one Undo step. Numeric opacity and layer names group changes until Enter or focus loss; Escape rolls the active edit back. Typing **42** in layer opacity means **42%**, not a fraction. Ctrl/Cmd+Shift+click selects the top visible layer with nonzero layer opacity and pixel alpha at that position, including locked layers. The gesture never paints, even if modifiers are released during its drag; Space navigation takes precedence. This is single-layer selection, not selection-mask editing.

Layer rows separate the visibility eye, active pen/additional-selection check, thumbnail, name and lock. Double-click a name to edit it; Enter/focus loss commits and Escape cancels. Click normally for one layer, Ctrl/Cmd-click or use the check column to toggle extra layers, and Shift-click for a range. Drag thumbnails/names to the insertion line to reorder the selected set; edge scrolling is supported. Escape, focus loss, stale data and release outside cancel the preview. Selection is session-local; Undo/Redo restores one active target. Alt-click an eye solos that layer; repeat restores the previous flags unless manually changed.

**Layer** and the palette share key-assignable commands and disabled reasons. Duplicate/Delete/Move/Visible/Locked act on the selected set; row eyes/locks affect only their row. Transfer to layer below clears but retains the source; Merge with layer below removes it. Both bake opacity and retain the lower name. Merge selected requires adjacent visible Normal layers. Merge visible keeps hidden layers, Merge visible to new layer preserves all originals, and Flatten image also removes hidden layers, retaining transparent background. Clear selected layers clears pixels without deleting rows. Combining/clearing locked targets is refused, while copying visible content may read locked originals. Unsupported blend/range combinations explain why they are disabled. Pair/selected merges can introduce small 8-bit rounding differences; merging all visible content or flattening matches the rendered bytes. New commands start unassigned and do not repeat. Layer actions pause during strokes, dialogs and palette/layer gestures. Preferences v13 reads versions 1–12 without resetting keys; old builds protect settings they cannot read.

New/Duplicate layer checks the 64-layer limit and 512 MiB logical-pixel and uncompressed-snapshot budgets before allocating another layer. Snapshot overhead and UTF-8 names count toward the latter limit, so a 4096² drawing can hold seven RGBA layers, not eight. Disabled controls explain the limit. Saving also checks the snapshot size before encoding; an oversized rejection leaves existing saved generations intact. These are logical limits, not process-memory guarantees; whole-layer copy-on-write, rendering and saving can still cause peaks.

### Alpha lock and layer identities

Use **레이어 → 레이어 설정 → 투명 픽셀 잠금**, the checker-lock property button or a row's independent checker-lock target. The property/menu action toggles the selected set; row flags affect only that row. Paint recolors existing pixels without changing alpha, including antialiased edges. Eraser paints white while preserving alpha; Blend/Blur/Smudge also preserve target alpha. Fully transparent pixels remain byte-identical. Full lock blocks painting independently. Clear/Transfer currently require alpha lock off; a configurable eraser policy, masks and groups remain future work. Flags support one-step Undo/Redo, saved projects and recovery.

Stable nonzero layer IDs survive reorder, rename, undo and restart. New/duplicate/result layers get new IDs; canceled or undone branches never recycle an issued ID. Selection, inline rename and solo restoration resolve their targets by ID rather than accidentally addressing a replacement row.

Use **레이어 → 레이어 설정 → 아래 레이어에 클리핑** or the first layer-property icon to limit display to the nearest unclipped layer below. Consecutive clipped layers share that base's coverage and opacity; they do not increase its alpha. A rose-colored bar marks clipped rows. Painting outside the base is retained and reappears when clipping is disabled. Hidden or missing bases show none of their clipped content; layer picking follows this coverage. New layers inserted inside a chain inherit clipping. Subset merges/transfers require the entire chain and base, while visible merging/copying and flattening bake the displayed result. The flag supports selected-set commands, an assignable shortcut, Undo/Redo and restart.

Artwork saves version 5 and reads versions 1–4; older builds cannot open newly saved projects. Version 4 IDs, issued-ID counter, alpha lock, clipping and pixels are preserved, with no mask initially. Keep independent backups before migrating. Preferences v18 reads versions 1–17; portable brush exports remain v8.

**레이어 → 레이어 마스크** creates a reveal-all or hide-all mask on the active layer, deletes/applies it, or toggles its enable state. Click the original or grayscale mask thumbnail to choose the drawing target; the selected target has a bright border, and a disabled mask has a red cross. Paint restores mask coverage regardless of drawing color; Erase hides it without changing original RGBA. Pressure, tips, spacing, spraying and both accumulation modes reuse the brush engine. Whole-stroke Undo/Redo, cancellation and save/reopen retain masks. Disabled masks and Blend/Blur/Smudge refuse mask drawing rather than silently editing content. Full lock blocks mask editing; alpha lock applies to content and prevents destructive mask application, but not mask painting. New/opened documents start on content. Selection-shaped creation and linked transforms remain unimplemented/disabled. Clear selected layers still explicitly clears original content while retaining masks. Transfer down refuses masked layers until its target semantics are implemented. Uniform masks allocate no coverage canvas; dense masks share one-channel buffers with History and saves.

**레이어 → 레이어 마스크 → 마스크 범위 표시** overlays the active mask's hidden coverage in violet, including disabled masks for diagnosis. It changes display only: document pixels, exports and picked colors remain unchanged. The navigator shares that diagnostic display. One session-local layer ID owns the overlay; selecting another layer hides it until that target is active again. Delete/apply, New/Open and restart clear the display flag. Toggling refreshes the existing drawing texture rather than keeping another full-canvas cache.

**편집 → 편집 대상 비우기** clears the active mask to reveal all when its thumbnail is selected, or clears selected content layers otherwise. Fresh keymaps bind Delete and Backspace; saved keymaps keep existing assignments, so use Shortcut settings to add the new command if needed. The mask submenu also has an explicit **마스크 비우기** action. Mask clearing retains original RGBA and the enabled flag, permits content alpha lock, refuses full lock, and supports Undo/Redo and save/reopen. An already clear mask is a no-op that preserves redo. Text fields keep ownership of Delete/Backspace.

### Raster color conversion

Edit's **선 색을 그리기색으로 변경** replaces the active content's visible RGB with the drawing color, preserving alpha. **휘도를 투명도로 변환** extracts black coverage from brightness, making white transparent and retaining partial alpha. Both operate on the whole active raster content, preserve masks/other layers, use one Undo step and support custom shortcuts. Select the content thumbnail first. Full lock blocks both; alpha lock permits recoloring but blocks brightness conversion. Selection-limited conversion and non-raster layer types remain unsupported.

## Development

See [architecture](docs/architecture.md), [testing](docs/testing.md), and [roadmap](docs/roadmap.md).

```text
apps/parfait/             Native application, panels, input and canvas view
crates/parfait-core/      Raster document, brush engine, undo/redo
crates/parfait-storage/   Versioned SQLite snapshots, worker, recovery tests
docs/                    Architecture, testing, roadmap
```

## Current limits

The prototype starts at 1024 × 768 and uses whole-layer CPU buffers with changed-region CPU compositing and partial texture updates during strokes. Large-canvas tiled rendering is planned. Physical pen pressure/tilt and device-specific behavior still need hardware validation. Cross-window drag docking, nested split layouts, ABR/SUT import, advanced mixing, selection, transforms, and liquify are not implemented. Linux remains unverified; macOS native checks do not establish physical tablet or multi-monitor support. See the [feature plan](docs/feature-plan.md) for remaining editing tools and UI groups.

## License

MIT. See [LICENSE](LICENSE). Contributions should include source provenance, tests for behavior changes, and no third-party assets without redistribution permission.

Toolbar icons use bundled Font Awesome Free 6.7.2 (SIL OFL 1.1 for the font). See [asset attribution](apps/parfait/assets/fontawesome/README.md) and Help → Third-party notices in the app.

All application-rendered text, including Korean, numbers and rulers, prefers bundled Pretendard Regular (SIL OFL 1.1). Unsupported glyphs retain fallbacks; icon glyphs and OS-owned dialogs use their separate fonts. See [Pretendard attribution](apps/parfait/assets/pretendard/README.md).

**표시** offers 100%, 200%, Fit canvas, Reset display and Rotate / flip. Flipping affects the view, not artwork; drawing and picking map back to document coordinates. Fit keeps rotation/flip; Reset display clears them. Default keys: Shift+H/V to flip, Shift+Left/Right to rotate, Ctrl/Cmd+Alt+0 for 100%, Ctrl/Cmd+0 to fit and Ctrl/Cmd+Shift+0 to reset. **표시 설정** (Ctrl/Cmd+K) saves ruler visibility, a pixel grid visible at 800%+, brush-cursor visibility, outside-canvas background brightness and rotation increment. Ctrl/Cmd+R toggles rulers; Ctrl/Cmd+Shift+G toggles the pixel grid. This grid is display-only, not snapping. F1 lists current shortcuts; Escape closes settings/help.

**편집 → 단축키 설정** (default Ctrl/Cmd+Alt/Option+Shift+K) edits 60 implemented commands with search, categories and up to four bindings each. Enter accepts a free captured key; conflicts require explicit **Replace assignment**. Shared tool keys cycle in catalog order without auto-repeat. **Apply & close** confirms local saving before changing active keys; Cancel/×/Escape discards the draft. Failed saves retain the previous keys and draft for retry. Menus, tooltips and help share the same bindings. Space gestures, temporary Alt picking, Enter/Escape and common OS quit keys remain reserved. OS interception cannot be bypassed.

### Layer blend modes

Choose the active layer's mode in the Layers palette, beside opacity: Normal, Multiply, Screen, Overlay, Darken, Lighten, Color dodge, Color burn, Hard light, Soft light, Difference, Exclusion, Add and Subtract. Modes affect the canvas, navigator and PNG export; changes support undo/redo and persist in projects and recovery snapshots.

Version 1 projects receive Normal layers; version 2 preserves its blend modes. Both receive stable IDs and alpha lock off. Saving writes format v3, which older Parfait builds cannot open. Blending currently uses stored sRGB channel values, not linear-light or color-managed compositing.

### Compact brush controls

The Tool property palette starts with a named preview followed by size, opacity, brush density and smoothing. Rows use thin filled bars, unframed editable numbers, stacked increment/decrement chevrons and horizontal dividers. Brush size uses a logarithmic track; numbers remain in document pixels. Other units are shown in tooltips. The bottom wrench (**Sub tool detail**) opens six categories: Brush size, Ink, Brush tip, Stroke, Correction and Pressure. The adjacent disk and curved-arrow icons save or restore the preset. Each numeric parameter has an eye button to show or hide it in Tool property. Choose from eleven implemented parameters, including minimum size, hardness, roundness, angle, spacing, color stretch and fixed blur radius. In Details, **Save preset** stores both values and property visibility; **Restore saved** restores both while retaining the current color. Settings and parameter help use fixed-height scrolling regions so longer descriptions and category changes do not move the footer. The footer options menu contains **Default property layout**, which resets only visibility. Old preferences and `.pbrush` files default to the original four rows. The active layout is also saved in local preferences without requiring a preset save.

The detail window uses a fixed vertical category list, the same parameter rows as the palette, a shared live preview, and a parameter guide that follows the hovered control. Smoothing belongs to Correction; spacing belongs to Stroke. Unsupported controls are disabled with explanations. Hand and Eyedropper show their own instructions. Changes apply immediately; Close, title-bar × and Escape retain edits. Escape dismisses a dynamics child before its parent detail window. Device calibration retains its separate Apply/Cancel behavior. Preview size is capped at 38 px in neutral ink and does not simulate erasing, device calibration or pointer smoothing. `--brush-details` opens the editor and `--brush-dynamics` opens size dynamics for UI inspection.
