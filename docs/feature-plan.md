# Editor feature plan

This is an independent product backlog. A listed item is not implemented unless its status says so. Each group needs its own design, fixtures, undo/save behavior and acceptance checks.

| Group | Current behavior | Follow-up work and acceptance criteria |
| --- | --- | --- |
| Workspace | Reorder/move side palettes, float inside app, resize, hide and persist layout | Named workspaces, dock tabs and independent OS windows; restore across changed monitor/DPI setups without losing palettes |
| Window chrome | Custom title bar and native viewport controls | Verify OS move/resize/maximize behavior on Windows, macOS, X11 and Wayland |
| Brush controls | Six presets; size, opacity, flow, spacing, hardness, aspect, angle, smoothing, pressure graph | Texture/scatter, tilt, stroke taper, dual tips and wet mixing; deterministic fixture strokes and latency budgets |
| Materials | Persistent image tips and `.pbrush` settings | Tags, thumbnails, folders and more formats; report unsupported parameters instead of silently discarding them |
| Layer editing | Raster layers, thumbnails, lock/visibility/opacity, reorder/duplicate/delete | Groups, clipping, alpha lock, masks and blend modes; validate compositing and undo across reopen |
| Selection and fill | Planned | Rectangle/lasso, grow/shrink/feather, contiguous fill and gap handling; transparent-edge and multi-layer reference fixtures |
| Transform | View zoom/rotation only | Destructive/non-destructive move, scale, rotate and perspective; explicit resampling and reversible edits |
| Rulers and guides | Coordinate rulers only | Straight/perspective/symmetry guides, snapping and guide management |
| Vector/text | Planned | Editable paths, stroke widths and text layers; font fallback, serialization and raster export consistency |
| Filters/liquify | Planned | Adjustments, blur/sharpen and local deformation; bounded preview cost, undo and saved-result parity |
| History/commands | Undo/redo and fixed shortcuts | Visible history, typed commands, editable shortcuts and customizable command bar |
| Extended workflows | Still images | Multi-document workspace, color management, richer interchange; animation and 3D are separate later scopes |

Prioritize reliable drawing, recovery and cross-platform input before extending the tool catalog. UI controls must represent implemented behavior; avoid empty buttons for future features.
