# Editor feature plan

This is an independent product backlog. A listed item is not implemented unless its status says so. Each group needs its own design, fixtures, undo/save behavior and acceptance checks.

Raster mask editing supports explicit reveal-all/hide-all creation, content/mask thumbnail selection, Paint/Erase coverage, enable/delete/apply, History and artwork v5 save/recovery. Selection-shaped creation and linked transforms remain planned; mask-area display and target-aware clearing are implemented. Unsupported mask operations refuse input rather than editing original content. Seven mask commands share menu/context/assignable-key dispatch; only the active layer is affected, even with additional layers checked.

| Group | Current behavior | Follow-up work and acceptance criteria |
| --- | --- | --- |
| Menus | Eight localized roots, 41 navigable submenus, 67 routes to working actions and 327 explicitly disabled planned/menu-unconnected leaves | Implement engines behind planned entries in dependency order, reusing typed commands and their input/Undo/save guards; menu presence is not implementation |
| Workspace | Stack rows or dock side by side, adjustable adjacent row/column splits, filled drop previews, in-app/native window hosts, edge resizing, hide and persist layout | Named workspaces, dock tabs, nested splits and cross-window drag docking; restore across changed monitor/DPI setups without losing palettes |
| Window chrome | Custom title bar and native viewport controls | Verify OS move/resize/maximize behavior on Windows, macOS, X11 and Wayland |
| Brush controls | Fourteen presets including capped marker, dotted pen, timed airbrush, erasers, color blend, local blur and spatial smudge; family rail, session-local settings, operation and opacity accumulation selectors, size/opacity/density, pressure-driven Gap, spraying/rate, hardness/aspect/angle, smoothing and pressure handles | Independent tool-family metadata, texture/scatter, velocity-driven spacing, ribbon, multi-tip repeat, tilt, taper, dual tips and pigment-adding wet mixing; real-device hold/latency validation |
| Local color editing | Same-layer average blending and spatial patch smudging with stretch; alpha-safe local box blur with automatic/fixed radius, shared tips and partial damage | Pigment-adding mixing, clone/reference layers, additional blur kernels and texture-preserving behavior during size changes; bounded scratch and saved-result parity |
| Materials | Persistent image tips and `.pbrush` settings, family/search/favorite filters, cached engine previews | User folders/tags, sorting and more formats; report unsupported parameters instead of silently discarding them |
| Layer editing | Raster layers with stable IDs, alpha lock, display-only clipping chains, editable scalar masks and exact v1–v4 migration to v5; thumbnails, visibility/opacity, group drag, range/toggle multiselection, solo eyes and inline rename; shared collection commands; transfer/merge below, adjacent Normal complete-chain merges, visible merge/copy, flatten and clear with Undo/reopen tests | Groups, selection-shaped masks, vector layers and ruler policies; configurable alpha-locked eraser policy and Clear/Transfer behavior; arbitrary-blend/noncontiguous or partial-chain selected merges; versioned type/mask storage |
| Selection and fill | Planned | Rectangle/lasso, grow/shrink/feather, contiguous fill and gap handling; transparent-edge and multi-layer reference fixtures |
| Transform | View zoom/rotation only | Destructive/non-destructive move, scale, rotate and perspective; explicit resampling and reversible edits |
| Rulers and guides | Document-anchored pixel rulers with 100-pixel major marks, pan/zoom-aware density and clipped horizontal/vertical labels | Straight/perspective/symmetry guides, snapping and guide management |
| Vector/text | Planned | Editable paths, stroke widths and text layers; font fallback, serialization and raster export consistency |
| Filters/liquify | Local blur brush only; document filter commands planned | Adjustments, document blur/sharpen and local deformation; bounded preview cost, undo and saved-result parity |
| History/commands | Named History palette with one Undo/Redo cursor, clickable retained future rows and no-op-safe transactions; 76 typed commands, active-only layer ordering, cyclic selection, standalone rename, editable bindings with conflict replacement, shared tool-key cycling and synchronized hints | Customizable command bar, separate Mac Control/Command mapping, hardware and other-OS key validation |
| Extended workflows | Still images | Multi-document workspace, color management, richer interchange; animation and 3D are separate later scopes |

Prioritize reliable drawing, recovery and cross-platform input before extending the tool catalog. UI controls must represent implemented behavior; avoid empty buttons for future features.

## Immediate usability milestone

1. Put lightweight/stability gates before more features: unchanged-preference serialization and excessive idle maintenance are removed. Next profile whole-layer copy-on-write, aggregate document/undo working-set limits and changed-library saving, preserving recovery and compatibility.
2. Keep canvas navigation and coordinate feedback consistent: document-anchored rulers are implemented; retain pan/zoom/input regressions and native checks.
3. Keep common footer alignment, property/palette control density, menu shortcut columns and narrow-width geometry consistent across existing screens. Continue native focus/DPI checks and validate response-graph gestures against observed behavior before further graph redesign.
4. Extend local color editing beyond the reservoir blend/box blur and add one independently specified texture or scatter feature at a time, including core pixels, persistence compatibility, actual output previews and measured cost. Keep unsupported settings explicit.
