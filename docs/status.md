# Implementation status

Capability snapshot for 2026-10-19, based on the application at `81e3883`. This documentation checkpoint adds no drawing features. A menu entry or a related underlying algorithm is not evidence that its named command or user setting works.

## Working scope

| Area | Implemented behavior | Important limits |
| --- | --- | --- |
| Workspace | Seven palettes, left/right docks, adjacent row/column resizing, internal floating and native windows, saved placement | No nested dock tree, workspace presets or cross-window drag docking |
| Brush controls | Size, opacity, density, pressure response with editable anchors/handles, minimum size, fixed angle/aspect, hardness, smoothing, pressure spacing, stationary spray and capped accumulation | Size and opacity share a response curve; smoothing is an event filter; tilt, velocity, texture, scatter, taper and dual brushes are absent |
| Pixel operations | Paint, Erase, average-reservoir Blend, local box Blur and spatial-patch Smudge | Blend/Smudge do not add drawing pigment; a local Blur brush is not a document filter |
| Brush library | Procedural and image-mask tips, presets, search, favorites, rename/duplicate/delete, supported `.pbrush` exchange | Family drafts are session-local; unsupported brush formats and settings are not interchangeable |
| Layers | Stable IDs, multiselection, row dragging, rename, visibility, full/alpha locks, 14 blend modes and clipping chains | Flat raster layers only; Paper is a locked white raster; some combine actions require visible Normal layers and complete adjacent chains |
| Masks | Content/mask targets, scalar Paint/Erase coverage, enable/delete/apply, violet area display and target-aware clearing | No selection-shaped creation, linked movement or mask-expression controls; display state is session-only |
| Raster conversions | Whole-active-content drawing-color replacement and brightness-to-opacity extraction, with lock/target guards and Undo | No selection-area restriction, editable vector paths or non-destructive layer-color effect |
| Viewing and input | Pan/zoom/rotation/flips, document-anchored rulers, pixel grid, HSV color selection and one-pixel composite picking | No editing transforms, guide engine, configurable pointer map or independent physical Control/Command bindings |
| History and files | Named Undo/Redo with retained future, local project save/open, PNG export, asynchronous recovery and validated snapshot fallback | New drawings have fixed dimensions; no multiple-document workspace, general image-layer import or color-managed export |

Text uses Pretendard as the preferred application font, with fallback fonts and a separate icon family. Native system dialogs are outside that font configuration. Menu/body localization, keyboard accessibility, DPI and monitor transitions still need broader coverage.

## Do not count these as implemented

- Selection tools and commands, including selection-limited drawing, clearing, conversions and mask creation.
- Vector and text layer models, path/control-point editing, vector erasing and selection-boundary path splitting.
- Fill, gradient and figure tools with their reference-layer and gap-handling options.
- Document blur/sharpen/adjustment/effect filters, pixel movement/resampling, perspective transforms and liquify.
- Layer folders, reference/draft types, mask-transform links and unrestricted subset merging.
- A unified preferences interface or user controls for History capacity, time grouping, clone-name format, display-quality mode and recovery enablement/interval.
- A live pointer-coordinate/memory Information palette. Rulers and development benchmarks do not provide that interface.
- Previous/next size-preset navigation. Current smaller/larger commands multiply or divide size by 1.2.
- Settings repair/backup selection UI. Preserving unsupported settings files and reporting an error is not a repair workflow.

The catalog has eight top-level menus, 41 submenus and 394 leaf routes: 69 working routes to 65 distinct commands, plus 325 disabled planned/menu-unconnected leaves. The assignable registry contains 78 commands, including commands without a top-menu route. These are interface counts, not a completion percentage.

## Reliability and performance boundaries

Recovery uses immutable snapshots, a bounded pending slot, revision acknowledgments, SHA validation, three retained generations, SQLite WAL and `synchronous=FULL`. Unsupported or damaged preferences are reported and protected from overwrite. These safeguards do not prove hardware power-loss durability or universal platform support.

The displayed local-save status tracks recovery, not a separate saved-project dirty marker. Manual project save and PNG export run synchronously on the UI path. Separating those states and measuring large-document stalls are high-priority follow-ups.

Whole-layer copy-on-write and full CPU compositions remain. The 512 MiB logical/snapshot budgets and History's 128 MiB logical policy are not total-process RAM limits; History retains at least one edit even for a large document. Some measured full compositions exceed a 16.7 ms frame budget. CPU microbenchmarks and executable size are not pen-to-display latency or RSS measurements. See [performance evidence](performance.md).

## Verification checkpoint

The application source at `81e3883` passes 273 workspace tests, with one opt-in benchmark ignored, formatting and all-target Clippy with warnings denied. Tests cover core pixels, no-op/Undo behavior, input ownership, format migration, persistence and process-termination recovery. Isolated native macOS checks additionally cover selected menu, drawing and normal-restart scenarios. This is not an exhaustive native test of every feature, physical tablet or operating system. See [testing](testing.md) and [platform status](platforms.md).

Artwork writes version 5 and reads versions 1–4; preferences write version 18 and read versions 1–17; portable brush exports remain version 8. Older application versions may reject newer saved state. Keep independent backups when migrating.

## Recommended sequence

1. Separate project dirty/recovery indicators and measure synchronous save/export stalls, idle CPU, startup, RSS and input/frame tails.
2. Build selection state and apply it consistently to existing editing operations.
3. Add independent parameter curves and one measured brush feature at a time, retaining pixel/Undo/save regressions.
4. Add layer folders and actual movement/transforms before mask linking.
5. Introduce vector/text data, versioned storage, rendering and editing as explicit features rather than raster aliases.

The broader backlog remains in [the feature plan](feature-plan.md). This checkpoint does not mark that backlog complete.
