# File workflow and image-import validation

2026-10-22, macOS arm64, Rust 1.99.1, local debug builds. No artwork was uploaded, no AI service was used, and no dependencies or models were downloaded. Windows/Linux native behavior and physical pen latency were not measured.

## Implemented scope

- File and application-menu exit share the `Quit` command and existing final-recovery/session-layout path. Unsaved open drawings remain recoverable after normal exit; explicit tab discard remains distinct.
- Original-save progress is derived from the captured document's pending job or overwrite prompt. Cancelling approval no longer leaves a stored “saving” notice. The original, saved baseline and unsaved edits remain unchanged.
- Final shutdown rechecks overwrite prompts after draining source-save results. A conflict discovered during the drain cannot set the clean marker.
- Periodic preferences persistence handles an empty workspace after the last tab closes without dereferencing a nonexistent active session.
- File → 가져오기 exposes PNG/JPEG import as a new document or raster layer. A new document retains image dimensions and alpha over hidden, pixel-free paper. Its source image is not a writable project binding.
- Layer insertion preserves original pixels, centers the image without scaling, and rejects images larger than the canvas instead of cropping. It is one undoable edit, honors folder insertion restrictions, targets the captured session/layer and rejects a changed or closed destination.

## Input and memory limits

One background decode job is permitted at a time. The encoded input is limited to 32 MiB, each dimension to 4096 pixels, and decoder allocation requests to 128 MiB. A 320 MiB conservative pixel-work reservation includes the job and remains charged after cancellation until its result is drained. Image decoder allocation limits are best effort; this is not an application RSS guarantee.

The input is read into a bounded local snapshot; its detected content, not merely its extension, must be PNG or JPEG. Dimensions are checked before full pixel decoding. JPEG orientation metadata is applied, and output is converted to 8-bit straight RGBA. ICC profile conversion, animated-image playback, cropping/resizing during import and other image formats are outside this change.

A load reservation does not evict undo history. Cancelled/invalid imports do not publish a document or edit. Successful insertion uses the existing history/storage budget and recovery paths. Closing the application during an active import requires finishing or cancelling it first; a cancelled worker never writes artwork to disk.

## Focused automated checks

- Core image import: exact centered placement, transparent margins, byte-preserving RGBA, hidden procedural paper, invalid dimensions/pixel lengths, oversized input and locked-folder rejection.
- Application import: PNG/JPEG detection, 16-bit PNG conversion, JPEG orientation, encoded/dimension limits, tab/layer/memory admission, cancellation and Escape, empty-workspace command availability, redo preservation, captured-target isolation, stale-state rejection, project save/readback and clean-session recovery.
- Source saves: cancellation and late-conflict regressions plus existing asynchronous edit/tab isolation, approval recheck, original-save failure and normal/abnormal restart coverage.
- Actual egui menus: shared exit on dirty and empty workspaces, catalog/command-index consistency and existing menu routes. The menu test harness now renders the menu bar where the production window does.
- Last-tab close: empty-workspace preference persistence and both existing save/discard/cancel close-flow tests.

Commands and final suite results are listed below. Opt-in workloads were not used as pen-latency evidence.

## Native macOS smoke check

An isolated temporary application bundle used `/tmp/parfait-import-qa-20261022/native-recovery`, separate from user documents and the ordinary recovery folder. Inputs were independently generated 96 × 64 PNG/JPEG fixtures.

1. Open File → 가져오기 → 이미지를 새 그림으로… and select the RGBA PNG through the native picker.
2. Verify a separate `rgba *` tab, `96 × 64 px`, hidden paper, visible checkerboard alpha and independent “파일 저장 필요” / “복구 저장됨” states.
3. Import the JPEG through 이미지를 레이어로…; verify a new `rgb` layer above `rgba` without another document tab.
4. Use Cmd+Z and Cmd+Shift+Z to remove and restore the imported layer.
5. Use Cmd+S and the native save panel to write `imported-native.parfait`; verify “파일 저장됨” and the new tab title.
6. Use File → Parfait 종료; the process exits. Read-only inspection of the session database confirms a clean marker and the saved document's source binding.

The rendered PNG, layer list and alpha were visually inspected through the application screenshot. Picker cancellation, malformed files, size refusal, overwrite cancellation, recovery restart and late-result isolation have automated coverage; not every one was repeated through native mouse input. Existing NFD filename rendering and broader OS/device coverage remain separate work.

## Existing test debt and fixture repair

The starting commit `ac65f8e` was independently rebuilt from a temporary archive and its application suite failed 22 tests (289 passed, 7 ignored). Failures included a stale menu harness, outdated counts/version/layout assertions and the last-tab preference panic. This change fixes the relevant menu harness/counts and empty-workspace panic rather than claiming a previously green baseline.

Two asynchronous local-preview tests also failed intermittently before image import was implemented and in the concluding full run. They are not image-import tests. Remaining unrelated UI/settings/layout failures are retained and reported, not suppressed.

The storage future-version fixture previously used database version 12, which the existing storage implementation already supports. It now uses unsupported version 13, and the unknown snapshot header is also `SNAP13`, so rejection checks exercise unsupported formats rather than a supported version or a truncated supported header. The storage implementation and on-disk format were not changed.

## Recorded commands and results

- `cargo fmt --all -- --check`: passes.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: passes.
- `cargo test -p parfait-core --offline image_import`: 3 passed.
- `cargo test -p parfait --offline image_import::tests -- --test-threads=1`: 7 passed.
- `cargo test -p parfait --offline menus::tests -- --test-threads=1`: 15 passed.
- `cargo test -p parfait --offline source_saves::tests -- --test-threads=1`: 8 passed.
- `cargo test -p parfait --offline document_tabs::close::tests -- --test-threads=1`: 2 passed.
- `cargo test -p parfait --offline recovery_runtime::tests -- --test-threads=1`: 7 passed.
- `cargo test -p parfait --offline project_state::tests -- --test-threads=1`: 8 passed.
- Last-tab preference regression and the existing empty-workspace admission regression: pass.
- `cargo test -p parfait-storage --offline -- --test-threads=1`: all 56 library and 10 integration tests pass, including four process-kill recovery cases and the session/migration boundary subprocess test.
- `cargo test --workspace --offline --no-fail-fast -- --test-threads=1`: **562 passed, 10 failed, 9 ignored**. Core and storage targets pass; the application target remains non-green because of the following existing tests. This result must not be presented as a passing workspace suite.

Remaining application failures:

```text
brush_widgets::tests::both_row_densities_keep_targets_separate_and_disabled_values_unchanged
brushes::tests::legacy_samples_migrate_and_arbitrary_curve_coordinates_survive_every_preset_path
document_tabs::tests::full_workspace_places_tabs_between_command_bar_and_canvas_inside_docks
image_edit::local_preview::tests::disabled_then_reenabled_preview_does_not_get_stuck_on_a_cancelled_request
image_edit::local_preview::tests::latest_preview_rejects_stale_parameters_and_confirmation_reuses_exact_buffers
layer_commands::tests::actual_menu_palette_and_keyboard_routes_produce_the_same_edit
layer_commands::tests::command_catalog_and_preferences_keep_keys_layout_and_unknown_originals
panels::tests::opacity_drag_from_existing_handle_is_one_undoable_edit
preferences::tests::legacy_cache_upgrades_and_future_preferences_remain_untouched
workspace::tests::drag_detaches_and_preferences_restore_workspace_and_brushes
```

The core/storage totals include existing pixel-compositing, selection, masks, legacy migration, corrupt-head fallback and process-crash recovery tests. Import and save-flow changes do not waive those checks. The two local-preview tests are timing-sensitive and can pass in separate runs; their failure is still recorded here.
