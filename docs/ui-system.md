# Workspace UI system

Establish consistent workspace behavior before adding more editing tools. A palette is one reusable shell around its contents, regardless of whether it is docked or floating.

## Palette contract

- A single 23 px header contains the drag handle/name and compact collapse and close controls. Floating hosts must not add a second title bar.
- The header controls are separate hit targets: clicking collapse/close must never begin a move.
- Drag the actual palette, preserving the pointer's grab offset, current width, expanded body height and in-session scroll offset. Dock width takes precedence when docking into a different column.
- The content renderer and vertical scrolling behavior are shared in both hosts. Closing hides a palette; it does not destroy its data. View → Palettes restores visibility.
- Collapse shows only the header and preserves expanded height. Placement, visibility and collapsed state persist; old layouts without collapse state open expanded.
- Show an insertion line for an existing dock target and an edge highlight for an empty side. Release commits placement; Escape or focus loss restores the pre-drag placement and order.
- Use the same bottom grip for resizing. In a floating host it adjusts both width and body height; docks share column width. Keep the floating header reachable inside the application bounds.

`workspace.rs` owns placement, drag state, shared header and body shell. `panels.rs` and `brushes.rs` supply contents, not separate floating-window chrome. The floating host is an egui Area, not an independent operating-system window. Scroll offset is session-local; document and brush settings do not belong to the host.

## Settings dialogs

Settings dialogs have a single title and visible dismissal actions outside scrolling content. Cancel, title close and Escape discard a draft; Apply commits it. Main window controls remain available while editor controls behind the dialog are disabled. Use the pressure calibration dialog as the current implementation.

## Next UI work

Prioritize consistent keyboard focus, docking-tab groups, accessible names for all header controls, touch target adaptation and multi-monitor OS windows before expanding the tool catalog. These are planned capabilities, not features of the current in-app palette host. Keep regression checks for drag continuity, docking order, cancellation, resizing, collapse/hide, reopening and preference compatibility.
