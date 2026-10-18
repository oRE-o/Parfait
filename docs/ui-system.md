# Workspace UI system

Establish consistent workspace behavior before adding more editing tools. A palette is one reusable shell around its contents, regardless of whether it is docked or floating.

## Palette contract

- A single 23 px header contains the drag handle/name and compact collapse and close controls. Floating hosts must not add a second title bar.
- The header controls are separate hit targets: clicking collapse/close must never begin a move.
- Drag the actual palette, preserving the pointer's grab offset, current width, expanded body height and in-session scroll offset. Dock width takes precedence when docking into a different column.
- The content renderer and vertical scrolling behavior are shared in both hosts. Closing hides a palette; it does not destroy its data. View → Palettes restores visibility.
- Collapse shows only the header and preserves expanded height. Placement, visibility and collapsed state persist; old layouts without collapse state open expanded.
- Show a filled footprint from the same row layout calculation used to render panels. Left/right target quarters insert within a row; the upper/lower center inserts a separate row. Release commits placement; Escape or focus loss restores the pre-drag placement and order.
- Use the same bottom grip for resizing. In a floating host it adjusts both width and body height; docks share column width. Keep the floating header reachable inside the application bounds.

`workspace.rs` owns placement, drag state, shared header and body shell. `panels.rs` and `brushes.rs` supply contents, not separate floating-window chrome. The in-app floating host is an egui Area. The separate-window host uses immediate egui viewports with the same shell and no extra native title bar. Header context menus open independent windows or return them to a side dock/in-app host. OS close hides only that palette; native geometry persists separately. Tab hides in-app panels without closing independent windows. Scroll offset is session-local; document and brush settings do not belong to the host.

## Settings dialogs

Settings dialogs have a single title and visible dismissal actions outside scrolling content. Cancel, title close and Escape discard a draft; Apply commits it. Main window controls remain available while editor controls behind the dialog are disabled. Use the pressure calibration dialog as the current implementation.

## Next UI work

Prioritize consistent keyboard focus, docking-tab groups, arbitrary nested split layouts, accessible names for all header controls, touch target adaptation and cross-window drag docking before expanding the tool catalog. These remain planned. Current docks support rows of equal-width cells, not arbitrary nested split trees. Independent windows use menu commands to dock back; dragging across OS window boundaries is not implemented. Multi-monitor/DPI and macOS/Linux behavior still need native validation. Keep regression checks for drag continuity, docking order, cancellation, resizing, collapse/hide, reopening and preference compatibility.

## Shared response-curve editor

`apps/parfait/src/curve_editor.rs` is the single UI module for response graphs. It provides a square gray plot, quarter-step grid, input/output labels, percent endpoints, a thin white response line and small violet square control points. It handles point hit testing, selection, dragging, optional insertion/deletion, focused Delete/Backspace and an optional live input marker. Each instance has its own stable UI ID and caller-owned selection.

Callers provide normalized points and constraints: fixed x coordinates versus editable points, fixed endpoints, nondecreasing output versus unrestricted output, maximum point count, labels and plot size. This permits decreasing responses for future tilt/speed settings without weakening pressure constraints. Invalid input is reported without editing it. The widget never owns persistence, device input or brush behavior.

Both device calibration and brush pressure now use this widget. Device calibration has up to 16 movable points with fixed endpoints; brush pressure keeps its existing five x positions and movable output values. Stored point formats are unchanged. Curves now use shape-preserving cubic Hermite interpolation shared with the brush engine: the line passes through each control point, joins smoothly and does not overshoot neighboring output values. Both increasing and decreasing segments are supported. Actual tilt/speed input dynamics remain future work. Existing nonlinear presets now evaluate smoothly between their saved nodes; identity curves remain unchanged.

Standard buttons, combo boxes and numeric controls use visible one-pixel outlines, lighter neutral fills and distinct hover/pressed states. Custom icon buttons use the same interaction visuals, including their border.
