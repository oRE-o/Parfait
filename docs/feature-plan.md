# Editor feature plan

This is an independent product backlog. A listed item is not implemented unless its status says so. Each group needs its own design, fixtures, undo/save behavior and acceptance checks.

| Group | Current behavior | Follow-up work and acceptance criteria |
| --- | --- | --- |
| Workspace | Stack rows or dock side by side, filled drop previews, in-app/native window hosts, resize, hide and persist layout | Named workspaces, dock tabs, nested splits and cross-window drag docking; restore across changed monitor/DPI setups without losing palettes |
| Window chrome | Custom title bar and native viewport controls | Verify OS move/resize/maximize behavior on Windows, macOS, X11 and Wayland |
| Brush controls | Ten presets including dotted pen, timed airbrush and erasers; family rail, session-local tool settings, size, opacity, flow, four Gap modes/custom spacing, continuous spraying/rate, hardness, aspect, angle, smoothing and pressure handles | Texture/scatter, pressure/velocity-driven spacing, ribbon, multi-tip repeat, tilt, stroke taper, dual tips and wet mixing; real-device hold/latency validation |
| Materials | Persistent image tips and `.pbrush` settings, family/search/favorite filters, cached engine previews | User folders/tags, sorting and more formats; report unsupported parameters instead of silently discarding them |
| Layer editing | Raster layers, thumbnails, lock/visibility/opacity, reorder/duplicate/delete, 14 blend modes | Groups, clipping, alpha lock and masks; validate compositing and undo across reopen |
| Selection and fill | Planned | Rectangle/lasso, grow/shrink/feather, contiguous fill and gap handling; transparent-edge and multi-layer reference fixtures |
| Transform | View zoom/rotation only | Destructive/non-destructive move, scale, rotate and perspective; explicit resampling and reversible edits |
| Rulers and guides | Document-anchored pixel rulers with 100-pixel major marks, pan/zoom-aware density and clipped horizontal/vertical labels | Straight/perspective/symmetry guides, snapping and guide management |
| Vector/text | Planned | Editable paths, stroke widths and text layers; font fallback, serialization and raster export consistency |
| Filters/liquify | Planned | Adjustments, blur/sharpen and local deformation; bounded preview cost, undo and saved-result parity |
| History/commands | Undo/redo and fixed shortcuts | Visible history, typed commands, editable shortcuts and customizable command bar |
| Extended workflows | Still images | Multi-document workspace, color management, richer interchange; animation and 3D are separate later scopes |

Prioritize reliable drawing, recovery and cross-platform input before extending the tool catalog. UI controls must represent implemented behavior; avoid empty buttons for future features.

## Immediate usability milestone

1. Put lightweight/stability gates before more features: unchanged-preference serialization and excessive idle maintenance are removed. Next profile whole-layer copy-on-write, aggregate document/undo working-set limits and changed-library saving, preserving recovery and compatibility.
2. Keep canvas navigation and coordinate feedback consistent: document-anchored rulers are implemented; retain pan/zoom/input regressions and native checks.
3. Specify the common property/palette control density, labels, focus and numeric editing contract, then validate response-graph gestures against observed reference behavior before further graph redesign.
4. Extend brush expression with one independently specified texture or scatter feature at a time, including core pixels, persistence compatibility, actual output previews and measured cost. Keep unsupported settings explicit.
