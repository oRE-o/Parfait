# Tool operations

Tool containers are independent of their input operation. Brushes can be assigned to any container, including navigation. Tool entry duplication preserves options; assignments, names, color slots and the active non-brush entry survive restart. The portable brush file format remains v8. The project format is unchanged.

| Container | Implemented input operations |
| --- | --- |
| Operation | Pick and drag an active raster/text/vector layer; double-click native content to edit it |
| Move | Pan; rotate; click/drag zoom in/out; reset rotation |
| Selection | Rectangle, ellipse, bounded freehand lasso; Replace/Add/Subtract/Intersect; Shift/Alt overrides |
| Auto select | Contiguous/global color regions from the visible composite, active layer or marked reference layers; color tolerance |
| Picker | Visible composite or active source layer |
| Fill | Contiguous/global regions from composite, active or marked reference layers; drawing color, opacity and transparent color |
| Gradient | Linear/radial between drawing and secondary colors |
| Figure | Line, rectangle, ellipse; outline/fill; Shift constrains angles/proportions |
| Text | Open the editable text editor at the clicked document position |
| Drawing containers | Existing paint/erase/blend/blur/smudge engine and image tips; independent geometric star/ring tips |

Canvas gestures record document/session/revision ownership. Escape, focus loss, tool switches and document changes cancel pending non-brush gestures. Pixel-changing operations use the existing history and memory preflight. Shape and region tools work on an editable visible raster layer; mask targets continue to use brushes/erasers. Moving currently acts on the active layer, not a collection. Text/vector movement updates source geometry. Folder/paper movement and a full object-transform gizmo are outside these input tools.

Region matching compares sRGB and alpha channels, treats fully transparent pixels as equivalent, and uses a scanline flood stack for contiguous regions. Reference input respects visibility and folder ancestry. Fill/gradient writes obey the existing pixel selection and preserve alpha when transparent-pixel locking is enabled. Transparent filling of an alpha-locked target is a no-op. Fill does not implement gap closing, region expansion, vector-boundary stopping or color-mixing materials. Selection geometry currently uses pixel-center coverage rather than a configurable antialias kernel. The lasso retains at most 4096 points. These operations are independently implemented and do not claim equivalence to another application's brush or region engine.

The hue/SV picker interpolates sRGB values, preserving hue through neutral/black selections. Main, secondary and transparent color buttons appear in both the rail and color palette. Transparent drawing uses erasing without changing a saved brush's operation.

Parameter rows use short tracks with separate numeric, arrow and dynamics targets. Ordered pointer events preserve release positions for coalesced slider drags. During pointer manipulation, brush sample regeneration and synchronous preference writes are deferred. Large full-canvas refreshes use one background snapshot/result at a time; intermediate property changes coalesce. Session/revision/overlay checks prevent old results overwriting newer synchronous strokes. Raw thumbnail caches compare source buffers instead of rebuilding on every property change. Select All uses four transformed boundary segments and retains canvas/thumbnail textures for selection-only edits. These are work-reduction and correctness changes, not measured physical pen-latency or cross-application performance claims.

Validation is intentionally focused: core region/fill/selection tests, category persistence and gesture tests, compact-list geometry, coalesced opacity input, formatting, compilation and Clippy. Native QA uses a separate data directory. Windows/Linux window dragging and physical tablet pressure still require device/platform verification.
