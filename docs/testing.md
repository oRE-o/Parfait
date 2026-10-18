# Testing

Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`.

Preference fast-path tests compare all saved-state changes against serialized records, ignore session-only search/drag/scroll caches, check old-version upgrades, protect future-version records and retry after validation/database failures. A temporary directory in place of the preferences database proves unchanged settings perform no I/O. Existing restart tests read actual SQLite JSON rather than a resident serialized cache. Idle-maintenance tests retain the fast schedule during pending saves, active strokes and close; timed-spray and forced-process recovery regressions remain mandatory. The ignored release-only large-library benchmark in [performance](performance.md) is opt-in and not linked into the application binary.

Spacing tests verify exact dab counts for every Gap mode, the 0.5 px floor, retained Fixed values, large custom gaps and identical pixels when a path is split into different movement events. Timed-spray tests compare 10 × 100 ms with 60 × 1/60 s, reject invalid time/rate, cap pause backlog, use current pressure, erase, respect layer locks and reconstruct partial/full pixels for all tip types. Canvas input tests hold without movement and ensure the frozen brush continues despite changing the global draft, then stops on release, focus/pointer loss, navigation and modal entry. Native stationary-hold behavior still requires real input/hardware verification.

UI tests click the actual Gap buttons and spray checkbox and keep Close reachable at 960 × 640. Preference versions 1–3 without new fields default to Fixed and spraying off. Version 4 portable/restart tests retain custom spacing, mode, spraying/rate and existing pressure handles. Use `--stroke-settings --data-dir <QA-directory>` to visually inspect active/disabled custom spacing and rate rows, the preview, Save/Restore and newly added Dotted pen/Continuous airbrush presets. Existing libraries add those only via Restore missing built-ins.

The Windows GitHub Actions configuration is provided as [ci-template.yml](ci-template.yml). CI is not enabled yet: publishing the workflow requires GitHub credentials with workflow permission. To enable it, place the template at `.github/workflows/ci.yml` and push using credentials with that permission.

Core tests cover alpha compositing, visibility, copy-on-write snapshots, continuous strokes, erasing, locked layers and undo/redo branching. Canvas tests cover rotated coordinate inversion and cursor-anchored zoom.

Headless egui input tests verify that hand-tool dragging pans without changing pixels, switching back to the pen draws, and a hue chosen while the current color is black is retained when saturation/value is raised. These tests supplement rendered-frame inspection; they do not replace native OS input or physical tablet validation.

Ruler tests cover fixed document-grid values, fractional pan at 5–1200% zoom, unchanged label/subdivision intervals while panning, negative and reversed projections, near-degenerate axes, non-finite/extreme input and bounded iteration. Rendered text tests check horizontal/rotated vertical label positions and clipping. A canvas input regression pans through real pointer events, checks the painted 100-pixel labels move with the view, and verifies unchanged artwork/history and no canvas texture upload. Native QA should compare a visible mark and document origin before/after Hand dragging, then zoom and resize without changing pixel units.

Additional regression checks cover Space taking over an existing stroke without leaving paint, Ctrl+Plus leaving UI scale unchanged, dragging a palette into a floating window, redocking it, resizing its height, and reopening persisted layout/pressure settings/image tips. Core tests verify pressure response changes both coverage and opacity, reject invalid coordinates and retain monotonicity specifically for device calibration.

Before release on each OS, also test custom-title dragging, double-click maximize/restore, all resize edges, minimize, and close with a pending save. Move a floating palette to each dock, reorder it, resize both its body and dock, then restart. Use a real tablet to confirm pressure telemetry, small-to-large strokes and driver mode; synthetic pressure tests do not prove device compatibility.

Storage tests round-trip snapshots, corrupt the newest payload and check fallback, and verify exclusive recovery ownership. Integration tests launch `recovery_probe` as a separate process, wait for a durable fixture milestone, forcibly kill it, and reopen the database:

1. Kill during an uncommitted replacement transaction: the prior brush pixels remain.
2. Kill after a committed layer edit: the latest layer count and brush pixels return.

These tests deliberately bypass normal shutdown. They establish process-crash recovery, not hardware power-loss guarantees.

For visual QA, `parfait --data-dir <temporary-directory> --screenshot <output.png>` exports the application's own rendered framebuffer after its first few frames. The app stays open for interaction. This option is inactive during normal startup.

Add `--brush-dynamics` to inspect size dynamics or `--brush-details` to inspect the category editor. Parameter-row tests click actual chevron/dynamics/track targets, check range clamping and keyboard endpoints, and verify dynamics clicks do not change values. Curve tests retain the final release-frame move, nudge output and switch fixed samples independently. Nested-dialog tests verify Escape dismisses the child first and that saved brush responses survive restart without changing artwork. Native checks should also edit a number directly, open size/opacity dynamics from their buttons, change a response and confirm the live preview and preset marker update.

Manual checks use `parfait --data-dir <temporary-directory>`: draw, switch layers, undo/redo, pan/zoom/rotate, save/export, wait for Saved locally, kill the app process, relaunch with the same directory, and compare pixels and layer state. Verify that a second app using that directory is rejected and the original continues working. Hardware pressure/tilt must be checked with real tablets before claiming device support.

Rendering regressions compare damaged-region patches with full composition across all procedural/image tips, erasing, pressure, opacity and canvas edges. A headless texture test reconstructs the image from actual upload deltas and checks checkerboard alignment plus full refresh after layer changes and undo. Run the release CPU benchmark described in [performance](performance.md); it is not a substitute for native pen-latency measurement.

Device calibration tests validate monotonic interpolation, rejection of sparse/constant samples, bounded collection, Cancel preserving state, Apply surviving restart and old preferences defaulting to identity. On a real tablet, open the pressure calibration window, draw light/firm strokes, suggest a curve, add/drag/delete points, test it, and verify Apply across restart. Confirm that mouse-only drawing does not produce a suggestion and Cancel/Escape/window close leave artwork and the previous curve unchanged. The test pad previews device calibration alone; individual brush dynamics apply afterward on the canvas.

Dialog regression tests click the actual cancel button at 960 × 640, check that it is on screen, and send Escape with a modified draft to verify dismissal without applying changes.

Workspace checks assert live detachment before pointer release, preserved width/body height, shared collapse/hide controls in both hosts, persisted hidden/collapsed state, legacy preference compatibility and Escape restoring placement/order/geometry.

A synthetic right-edge drag test compares the preview footprint with the actual side-by-side slot after drop. Additional checks cover row identity when moving a group owner and native placement/geometry preference round trips. Windows runtime QA observed a distinct palette OS window and closed it via Alt+F4 while the root window remained open and the palette visibility persisted as false. Native cross-monitor/DPI, resize and all target operating systems still require broader manual checks.

Shared curve-editor tests cover declining responses, pressure monotonicity, fixed-x/endpoints and independent selection/edit/delete behavior for two adjacent graphs. Calibration interaction tests exercise the same widget, including fast point dragging.

Variable brush-curve tests insert at non-quarter inputs, retain new points on release, move in both axes, use Shift nudging, preserve endpoints and enforce the 16-point cap. Drag-out deletion occurs on release, not crossing the border alone. A minimum-window test adds a point and clicks the still-visible Close button. JSON tests migrate version 1 arrays, read version 2 coordinates, preserve arbitrary/falling coordinates and manual controls across preset restore, family switching, version 3 portable records and restart, and reject malformed curves. Core tests verify actual dab pixels at arbitrary input nodes; damaged-region equivalence covers a falling response across every tip and erasing mode. Synthetic tests do not establish physical tablet behavior.

A batched-input regression sends press, movement and release in one frame: point insertion, 2D movement and outside-drop deletion must all retain their result. Disabled graph controls ignore the same batch. The editor processes ordered pointer events rather than relying solely on the final per-frame button state.

Blend tests cover known channel values, transparent/partial alpha, all-mode partial/full equivalence, undo/redo, canvas texture invalidation, v1 migration, mixed-generation fallback, saved-mode roundtrips and mode preservation after process termination.

Smooth-response tests verify exact control points, identity, continuous tangents, monotonicity and bounded output across rising/falling/flat and tightly spaced segments, plus equality between brush and calibration evaluation.

Tangent-handle tests verify automatic-to-Bezier shape equivalence, stationary anchors while changing actual dab pixels, invalid handles, and preservation/translation across topology edits. UI tests drag a round handle with press/move/release in one frame, ensure selection alone leaves automatic data unchanged, route arrow keys to the handle, and clamp outside drags without deleting anchors. Portable v3 and restart tests preserve manual controls; v1 arrays and v2 coordinates remain readable. Region/full equivalence includes explicit Bezier responses. Native checks should select square anchors, drag both round handles, edit Handle X/Y, restore Auto curvature, save/reopen and keep Close reachable at minimum window size. Hardware pressure/latency remains separate.

Brush detail tests render every category at a small desktop size, validate the active brush, verify preview texture reuse and updated settings, preserve artwork, save the active preset, and close via Escape without reverting live edits.

Canvas zoom regression tests feed actual keyboard, wheel and pinch events: Ctrl plus/equals/minus changes canvas zoom while UI scale stays fixed; plain and Ctrl-wheel zoom in both directions, preserve the pointer anchor, and do nothing outside the canvas. Keyboard zoom keeps the viewport center fixed and is blocked by brush details.

Modifier fallback tests cover Ctrl+B/E/brackets/Tab, registered Ctrl+Shift+N and Ctrl+Z precedence, Ctrl+middle-button panning without zooming, and normal pen input while Ctrl is held.

Brush-library checks cover independent unsaved pen/eraser settings, color preservation, preset restore, painting-preset selection exiting erase mode, composed search/family/favorite filters, favorites surviving save/restart, old preference and `.pbrush` records without new fields, missing eraser initialization, and distinct engine-rendered preview pixels. Native UI checks should additionally exercise rail/filter selection, search text focus blocking shortcuts, favorites, Manage, modified markers, Save/Restore and each detail category. Inspect color-wheel/navigation controls at default and resized palette heights. Family drafts are session-local; only explicitly saved presets and the active brush survive restart.

Compact-library input tests verify header controls fit 190–360-point palette widths, independent row/favorite hit regions, 40-point row height, explicit search focus and clearing, composed favorites/search filters, a browse-only family dropdown and the retained Manage actions. Preview tests distinguish transparent light list samples from unchanged neutral-background property samples. Native QA should also check flat rail selection/hover/focus, long-name truncation, keyboard tool switching, search shortcuts and persisted favorites after restart.

Detail regressions render all six categories at 960 × 640, assert that category buttons remain visible and vertically separated, and click the actual Spacing eye target. This checks layout-only edits, unchanged brush values/artwork, preset restore, family switching, preferences/preset visibility after restart and compatibility with older records missing property layouts. Native checks should pin a hidden property, close Details, verify its palette row, save/reopen the preset, and confirm that an unsupported tip parameter remains disabled while its visibility can still be configured.

Parameter-row tests exercise both 42-point palette and 28-point detail densities: independent number/chevron/dynamics targets, logarithmic bar click/drag, keyboard bounds, deferred typed-value commit and disabled-value preservation. Geometry checks keep bar, number, arrows and dynamics separate in a 210-point test viewport. Native checks should compare the compact rows with the unchanged palette rows and verify the overlaid preset name, short category guidance, parameter hover help and right-aligned Gap buttons.
