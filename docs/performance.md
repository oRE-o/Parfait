# Stroke performance

## Solid Layer Foundation

Fill and Paper retain three color bytes and no RGBA canvas. They add no dependency, texture cache or worker. Non-raster composition reuses one row per solid/folder node; a maximum-width 64-node test checks actual Vec capacities at no more than 1 MiB total and verifies allocation identity through composition. Existing raster-only composition retains its flat path. Masks still reserve a scalar channel in the conservative logical budget, including Uniform masks. The release executable grows from 13,126,736 to 13,143,792 bytes (+17,056); this is not process RSS.

Run `cargo run --release -p parfait-core --example procedural_bench`. On Apple M1/macOS 15.7.5 arm64/Rust 1.99.1, nine samples compare a solid Paper, a raster, a Fill with its default revealing mask and a translucent folder with an equivalent independently materialized raster document. Every result must match byte-for-byte, including a 64² central region. At 2048², procedural/baked RGBA payloads total 16/48 MiB; row scratch is 24 KiB for full composition and 768 bytes for the 64² region. These figures exclude masks, output, History and GUI/GPU memory. Full isolated composition median/p95 is 98.0455/101.1086 ms versus baked 97.6794/99.6727; Through is 89.9933/90.9820 versus 90.1212/94.6159. Partial-region p95 is 0.1012–0.1046 ms. Full-refresh latency remains a bottleneck; reducing source payload is not a claim of faster composition or a 16.7-ms frame.

Six sequential before/after selection-free stroke runs preserve all ten checksums and scratch capacities. The first three pairs use before/after order, the final three reverse it. Median run p95 values include Paint/PerStroke 2.3616→2.7594 ms and Smudge/PerStroke 3.8004→4.1226 ms, with substantial tail variability. Typical p50 costs stay closer, but these mixed results do not establish zero timing regression. No brush algorithm or additional selection-free stroke buffer is introduced by the solid types. Repeat isolated tail measurements before making responsiveness claims; this CPU workload is not physical pen latency.

## Multi-document 1B State Identity

The retained 1B scope adds History state IDs and an explicit streaming digest API, not tabs or rendering algorithms. The application never invokes the digest during pointer movement or modified-status rendering. Transaction start reserves entropy once per edit transaction, not once per pointer move. Artwork encoding and autosave durability remain unchanged; original-save baseline migration is deferred to 4A.

The existing 512×384 folder/mask/selection workload below was repeated on the start-of-task 1A source snapshot and retained 1B source, using Rust 1.99.1 / macOS arm64 and the workspace release profile. Each executable was preserved before alternating three pairs (the second reversed), with no concurrent builds. Values are minimum–maximum across all three runs, not pooled percentiles. No timing/RSS acceptance tolerance was fixed beforehand, so these are diagnostic observations, not a no-regression or latency claim.

| Metric | Before 1B | After 1B |
| --- | ---: | ---: |
| Canvas CPU frame p50, ms | 0.005917–0.008500 | 0.006584–0.007625 |
| Canvas CPU frame p95, ms | 0.026750–0.032209 | 0.032042–0.034042 |
| Canvas CPU frame p99, ms | 0.028292–0.035666 | 0.038583–0.055000 |
| Canvas CPU frame maximum, ms | 0.056834–0.078875 | 0.065583–0.096750 |
| Synchronous original-save p50, ms | 15.942167–16.571542 | 15.849167–16.617000 |
| Synchronous original-save p95, ms | 17.042000–17.717542 | 16.519375–17.264500 |
| Complete headless process peak RSS, MiB | 38.578–40.547 | 38.078–40.469 |

All six runs retain raster checksum `32465291941967`, complete composite checksum `63671080545421` and logical pixel bytes `2162688`, plus exact byte/readback assertions. The timer encloses headless egui canvas CPU work (256 updates); nine synchronous original-save samples include commit/checkpoint. It excludes the native event loop, full app UI, GPU presentation and physical input. Peak RSS is the entire test child's high-water mark from a fresh `resource.getrusage` wrapper, not a native GUI working set or the logical buffer budget. The before source/binaries were retained only in temporary QA storage, not a new historical overlay in this repository; the current workload remains reproducible with the command below.

```sh
cargo test --release -p parfait --offline multi_document_baseline_release_workload -- --ignored --nocapture --test-threads=1
```

## Multi-document 1A Session Extraction

1A moves the single live document and its editing state into `DocumentSession`, and render caches into one `WindowRenderCache`. It adds only small identity/ownership fields and egui identity scopes; it does not change the pixel algorithms, history limit, storage encoding, snapshot policy or worker count. Moving a session preserves the existing pixel/mask/selection Arcs, verified by pointer identity and unchanged strong counts. There is still one document and one fixed autosave worker. This is not completion of multiple tabs or global memory budgets.

The preserved before executable links base `e734ff62ca3b6af3725cdf7047ad338674450ded` plus only the baseline test/registration. The after executable links the uncommitted 1A working tree. Both use the workspace's release profile (thin LTO), Rust 1.99.1 and macOS 26.4.1 arm64. The exact hardware model could not be queried in the sandbox; this is not the named modest reference-machine acceptance run. The former raw evidence and historical overlay were removed from this checkout. The following values remain historical observations; the old before-build is not reproducibly archived in the current tree.

The workload uses a deterministic 512×384 document, two RGBA raster layers, an isolated folder, Dense child mask, Uniform-193 folder mask and fractional Dense selection. Logical pixel reservations are 2,162,688 bytes. An independent integer oracle verifies the initial complete composite. After three warm frames, one 32-pixel pointer stroke runs for 256 updates, 1.5 document pixels apart at synthetic 60 Hz timestamps. Unchanged/no-dab updates are included. Each timer encloses `egui::Context::run_ui` of the real canvas handler: input processing, core stroke work, CPU composition, boundary preparation and texture-delta creation. Texture-delta cleanup, final checksum verification and setup are outside the timer. This excludes eframe/native event delivery, the rest of the app UI, tessellation/GPU presentation and hardware input. It is a **headless canvas CPU frame**, not end-to-end pen latency or the native app's full frame time.

Nine original-save samples call the real synchronous `save_project_to`, including path validation, immutable snapshot preparation, encoding/compression, SQLite commit/clean/checkpoint and autosave submission. Each saved document is reopened and compared outside the timer, normalizing only the independent disk revision. Original autosave remains enabled; durability is not disabled to improve timings. Verification/readback and fixture cleanup are untimed. A future asynchronous-save workload must wait for the actual successful original commit, not time queue submission as a completed save.

Three final before/after pairs were interleaved, reversing pair two, without concurrent builds, the native QA app or UI automation. Other desktop apps remained running. Each table cell is the minimum–maximum across **all three runs**, not pooled percentiles or selected best runs:

| Metric | Before | After 1A |
| --- | ---: | ---: |
| Canvas CPU frame p50, ms | 0.005792–0.005958 | 0.006084–0.016083 |
| Canvas CPU frame p95, ms | 0.026041–0.028333 | 0.026417–0.082917 |
| Canvas CPU frame p99, ms | 0.028417–0.038625 | 0.027291–0.121292 |
| Canvas CPU frame maximum, ms | 0.053333–0.112417 | 0.052666–0.478250 |
| Synchronous original-save p50, ms | 15.875333–18.559583 | 15.564666–21.601958 |
| Synchronous original-save p95, ms | 17.344083–40.992667 | 16.423208–36.075958 |
| Complete headless test-process peak RSS, MiB | 38.71875–40.890625 | 37.859375–40.390625 |

The third after run has visibly higher frame costs; both variants also have save-time outliers. These observations are retained, not attributed to a proven cause or hidden as noise. No pre-agreed timing/RSS tolerance or isolated-device baseline exists, so timing effects remain **unjudged**, not a no-regression or speedup result. In particular, this does not claim the native 16.7 ms target is met. The scope is one small document, not large-brush/layer/memory-pressure coverage.

The raster checksum is 32465291941967 and complete composite checksum is 63671080545421 in every before/after run; exact decoded artwork assertions also pass. Nearest-rank percentiles use 256 frame samples and nine save samples per run. Save p95 is consequently that run's maximum, and save p99 is intentionally not reported. Peak RSS comes from `resource.getrusage(RUSAGE_CHILDREN).ru_maxrss`, in bytes on macOS, with a fresh Python wrapper for each single test child. It includes fixture construction, readback, headless egui, history, caches and worker memory across the entire test process; it is not the live native GUI/GPU working set or the document's buffer budget.

```sh
cargo test --release -p parfait multi_document_baseline_release_workload -- --ignored --nocapture --test-threads=1
```

For per-executable peak RSS, preserve the executable from `cargo test --no-run`, then run a fresh wrapper for each sample:

```sh
python3 - "$test_executable" <<'PY'
import resource
import subprocess
import sys
subprocess.run([
    sys.argv[1], "multi_document_baseline_release_workload",
    "--ignored", "--nocapture", "--test-threads=1",
], check=True)
print("max_rss_bytes", resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss)
PY
```

Earlier exploratory measurements overlapped builds and are excluded from the final six-run evidence. `/usr/bin/time -l` could not query the sandbox's kernel clock service; the independent child-resource wrapper supplies the reported RSS instead. A separate native release startup/framebuffer smoke check is described in [testing](testing.md#multi-document-1a-single-session-ownership-extraction). It does not provide native input timing, physical-tablet compatibility or Windows/Linux results.


## Selection Boundary Display

The selection overlay adds no runtime dependency, worker or image texture. It caches two vector dash phases, bounded to 16,384 quads per phase and 3.25 MiB combined vertex/index payload. Temporary contour segments are also bounded by that quad budget. Coverage is shared, not copied into the cache. Viewport-limited document-grid traversal uses a 262,144-cell preflight with conservative margins; power-of-two display LOD reduces dense boundaries and is labelled on the canvas. This can simplify small islands at low zoom or very high complexity, but never changes writing coverage. Unrelated pixel edits reuse the mesh; animation only chooses an Arc-backed phase every 200 ms. The unsigned release executable is 13,126,736 bytes, up 33,312 bytes from the preceding integrated engine checkpoint; this is binary size, not total process memory.

Run `cargo test -p parfait --release selection_boundary_release_workload -- --ignored --nocapture`. On Apple M1/macOS 15.7.5 arm64/Rust 1.99.1, 21 rebuild samples per case produce these median/p95 CPU times: 512² ring 0.7427/0.8485 ms, 512² checker 0.8800/0.9382 ms, 2048² ring 0.7543/0.8340 ms and 2048² checker 0.6365/0.6562 ms. The 800×600-point viewport uses zoom 1 or 0.25 and two physical pixels per point. Chosen grid steps are 2 and 8; mesh capacities total 107,328–108,160 bytes. These coarse checker samples intentionally exercise simplified display, not exact per-pixel contours. Cached Arc phase cloning alone measures 5.23–8.90 ns per iteration, not a complete UI frame. These are added CPU costs, not speedups, measured working-set memory or physical pen latency.

## Pixel Selection Foundation

No runtime dependency, worker or canvas-sized selection buffer is added to selection-free drawings. Active Dense selection uses one shared byte per pixel, at most 16 MiB at 4096²; Uniform selection has no canvas payload. Explicit layer-geometry extraction uses one scalar row per folder, at most 256 KiB, plus its ordinary scalar output. Mask creation can share selection coverage, while inversion/combination explicitly allocate a new scalar result. These are payload/scratch bounds, not total RSS; existing pixel COW, capped-stroke scratch and retained History still apply. Selection is borrowed directly by writers, not cloned into each Dab.

Run `cargo run --release -p parfait-core --example selection_bench`, optionally with `-- --selected`. On Apple M1/macOS 15.7.5 arm64/Rust 1.99.1, three interleaved before/after runs each measure 25 complete 64-update strokes on a 512² patterned raster, diameter 48, opacity 0.65 and flow 0.6. Timing includes the first pixel COW but excludes fixture construction, cloning, coverage setup, checksum/unchanged-hole verification, composition, UI, GPU and storage. The baseline links the preceding core with the same workload module. All ten selection-free checksums agree before/after across all runs. Dense coverage is 0 in one third of pixels and 128 elsewhere; it performs different work and is not a speedup baseline.

| Operation | Mode | Before p50 ms range | After p50 ms range | Dense p50 ms range |
| --- | --- | --- | --- | --- |
| Paint | Per dab | 1.5352–1.5559 | 1.5520–1.5776 | 1.4095–1.4535 |
| Paint | Capped | 2.0432–2.0647 | 2.1002–2.1131 | 1.5971–1.6412 |
| Erase | Per dab | 1.7401–1.7626 | 1.8053–1.8251 | 1.6206–1.6287 |
| Erase | Capped | 2.2977–2.3218 | 2.4553–2.4665 | 1.8394–1.8480 |
| Blend | Per dab | 1.8657–1.8783 | 1.8626–1.8742 | 2.2645–2.2716 |
| Blend | Capped setting | 1.8672–1.8826 | 1.8640–1.8776 | 2.2670–2.2982 |
| Blur | Per dab | 2.3524–2.3833 | 2.3337–2.3577 | 3.7246–3.7818 |
| Blur | Capped setting | 2.3559–2.3963 | 2.3495–2.3935 | 3.7320–3.8095 |
| Smudge | Per dab | 3.6271–3.7449 | 3.6380–3.6858 | 3.7399–3.8143 |
| Smudge | Capped setting | 3.7336–3.8345 | 3.7175–3.9440 | 3.7926–3.8606 |

Early prototypes increased selection-free Blend/Blur cost; final writers dispatch outside those loops to compile-time-specialized paths. Some unselected overhead remains, notably capped Erase, and active Dense Blur is more expensive. These CPU samples do not establish zero regression, frame rate, pen latency or product parity. Source-selection generation is not timed by this workload. The release executable is 13,093,424 bytes, up 16,736 from the preceding build; all 14 file-backed sections match the signed native QA executable. Size is not RAM.

## Through Folder Rows

Through composition reuses one RGBA row per folder, using that row as a before-image only when group opacity or an enabled mask needs interpolation. It adds no folder-sized image, dependency or background task. At 64 nodes and width 4096, row pixels remain bounded by 1 MiB, excluding the normal output and vector metadata. A maximum-depth regression checks retained buffer pointers/capacities after traversal. Artwork v8 changes the allowed enum range, not metadata size or the logical 512 MiB limits.

The arm64 release executable is 13,076,688 bytes, up 16,624 bytes from the preceding 13,060,064-byte folder-UI build. All 14 file-backed sections match the signed native QA executable. Artifact size is not process memory or input latency.

Run `cargo test --release -p parfait-core composition_timing_workload -- --ignored --nocapture`. On Apple M1, macOS 15.7.5 arm64 and Rust 1.99.1, 25 full compositions per case use eight raster nodes in two folders. The preserved preceding test executable and current executable give the following isolated-folder comparison. Every result exactly matches the independently baked pixels.

| Square size | Before p50/p95 ms | After p50/p95 ms |
| --- | --- | --- |
| 256 | 3.7845 / 3.9070 | 3.8206 / 4.0834 |
| 512 | 15.4467 / 17.6431 | 14.4901 / 14.6707 |

A separate Through workload uses alternating Normal/Multiply raster modes. Full-strength groups exactly match flat composition; an outer 63% opacity and Uniform-193 mask match an independent f64 premultiplied oracle within one byte. Through p50/p95 is 3.8485/3.9647 ms full-strength and 4.2587/4.3835 ms attenuated at 256, and 15.8837/16.2660 and 17.1574/17.4703 ms at 512. Two row buffers occupy 2048/4096 bytes. These different workloads and small noisy samples do not establish a general speedup, frame-rate guarantee, total RSS or physical pen latency. Larger documents and many active folders still require measurement.

## Folder UI Cost Check

The arm64 macOS release executable is 13,060,064 bytes, up 50,512 bytes from the preceding 13,009,552-byte folder foundation. No dependency is added. Tree movement, grouping and duplication share pixel/mask buffers; view folding stores at most 64 stable IDs and never creates document History. Folder masks reuse scalar coverage painting, not an RGBA folder canvas. This is code/data accounting, not a process-memory or input-latency guarantee.

A repeat of the 25-composition release workload gives folder p50/p95 3.8286/5.8960 ms at 256 square and 14.8810/15.5476 ms at 512 square, with exact expected pixels and 2048/4096 bytes of row scratch. The independently baked single raster gives 0.6744/0.7932 and 1.3008/1.3489 ms respectively. It does less work and is not an equivalent live-layer baseline. These samples do not establish a speedup over the prior noisy samples or physical pen latency.

## Isolated Folder Rows

Flat documents retain the preceding path. Folders store no RGBA image; isolated composition uses one reusable tightly packed row per folder, at most 1 MiB of row pixels for 64 nodes at width 4096, apart from the normal output and vector metadata. This is scratch accounting, not total process RSS. Grouping/ungrouping move metadata and share existing pixel/mask buffers. Ancestor lookup and layout metadata remain bounded by 64 nodes.

Run `cargo test --release -p parfait-core folder_composition_timing_workload -- --ignored --nocapture`. On arm64 macOS 15.7.5, a preserved before/after run of 25 full compositions uses eight raster nodes inside two isolated folders. Reusing the existing no-clipping row routine changes folder p50/p95 from 4.9910/8.2141 to 4.0842/5.7672 ms at 256 square, and 20.7692/22.7041 to 15.2735/16.1327 ms at 512 square. Every measured result equals the independently pre-baked expected pixels. This small two-run comparison includes timing noise and does not establish physical pen latency or a frame-rate guarantee.

The same workload's single pre-baked raster takes p50/p95 0.6028/0.7027 ms at 256 and 1.2885/1.3995 ms at 512 after the change. It represents less work, not an equivalent live-layer baseline or a claimed folder speedup. The two row buffers occupy 2048/4096 bytes respectively. These historical measurements predate Through blending, measured separately above.

The arm64 release executable is 13,009,552 bytes, 16,912 above the preceding control-layout build. Artifact size is not working-set memory or pen latency. The foundation does not add runtime dependencies or a background service.

## Layer Control Layout

The UI-only layout increment adds no dependency, persisted field, background work or canvas-sized buffer. The percent track builds a small mesh proportional to its displayed width. Pixel rendering and storage algorithms are unchanged. The arm64 release executable is 12,992,640 bytes, 16,528 above the preceding layer-role build; all 14 file-backed sections match the signed final QA app. This artifact-size measurement is not a runtime, memory or pen-latency result. No new runtime speedup is claimed.

## Layer Roles And Details

Reference/draft roles add two persisted booleans per layer, at most 128 snapshot bytes across 64 layers. Metadata, History, duplicate and saved-state comparisons share existing pixel/mask buffers. The modal retains one shallow layer draft while editing is blocked and allocates no canvas-sized preview. PNG export bypasses role processing when there is no visible draft; otherwise it shallow-clones document metadata, hides drafts in that copy and reuses the compositor. It does not remove structural clipping bases or copy source pixel arrays. The normal RGBA output allocation and full export composition remain, and role edits currently follow ordinary revision-driven display invalidation. No runtime dependency, background service or persistent image cache is added.

The preceding layer-role arm64 release executable is 12,976,112 bytes, 49,904 above the save-state baseline. This is artifact size, not RAM or pen latency. All 14 file-backed Mach-O sections match its signed isolated QA executable. No new runtime speedup or frame/memory baseline is claimed. The synchronous-I/O measurements below describe the earlier v5-format revision `60650ea`; v6 adds two bytes per layer, so their serialized byte counts are historical rather than current-format assertions.

## Project Save State

The saved-project marker compares metadata and shared-buffer identity across at most 64 layers. It does not scan pixel/mask bytes, serialize or clone a document for each comparison. One successful-save/open baseline shares its buffers with the document initially, but can keep up to one previous logical document alive after edits and History eviction. The existing 512 MiB logical limit is not a total RSS bound; retained baseline data is not free memory. No worker, periodic task, canvas cache or runtime dependency is added. Manual project writes, pre-replacement backups and PNG export remain synchronous; measuring and reducing their UI stalls is still roadmap work. No new pen-latency or memory-speedup claim follows from this increment.

The 2026-10-20 arm64 unsigned release executable is 12,926,208 bytes, 256 bytes above the preceding raster-conversion build. This is artifact size, not memory use or speed. The signed isolated QA executable matches all 14 file-backed Mach-O sections of that release build; signature/load-command differences are excluded from the comparison.

## Synchronous Project And PNG Baseline

Run `cargo test --release -p parfait synchronous_project_and_png_baseline -- --ignored --nocapture`. On Apple M1/macOS 15.7.5/arm64/Rust 1.99.1, three serial process runs take three samples per case. Fixtures fill every layer with either a structured pattern or fixed-seed noise and include one dense mask. Their logical sizes are 7,077,888 and 53,477,376 bytes; serialized snapshot sizes are 7,078,057 and 53,477,633 bytes. These synthetic fixtures bracket different compression costs, not typical artwork or the largest supported document.

| Fixture | Fresh project mean ms range | Existing project mean ms range | PNG mean ms range |
| --- | ---: | ---: | ---: |
| 1024×768, 2 layers, structured | 30.02–32.38 | 59.88–60.61 | 9.98–10.36 |
| 1024×768, 2 layers, noise | 115.99–148.28 | 159.72–194.24 | 16.66–20.80 |
| 2048×1536, 4 layers, structured | 221.64–234.73 | 451.45–489.60 | 73.12–76.45 |
| 2048×1536, 4 layers, noise | 1023.85–1241.37 | 1403.55–1514.48 | 100.22–111.46 |

Project timing calls the application's actual synchronous writer, including open, existing snapshot read/validation when present, encoding/compression, FULL SQLite commit, clean marking and checkpoint. PNG timing includes full composition, encoding and the current ordinary image-file write; it does not have the project's SQLite durability contract. Creation of input buffers, roundtrip verification and cleanup are outside timers. Every project and decoded PNG matches its expected bytes, and all four composite checksums repeat across the three runs. The largest overwrite sample is 1596.50 ms. Three samples do not establish p95/p99.

The calls currently run on the UI thread, making project saving the first background-job priority, followed by PNG export. These numbers measure those synchronous workloads, not actual event-to-display latency, UI frames, concurrent autosave or process RSS. Other desktop apps were present; local filesystem/cache and limited free disk space affect results. An initial larger noisy fixture encountered a full disk; those failed/overlapping process runs are excluded. Final runs use one process at a time, four rather than eight large layers and immediate cleanup of verified test-owned output files. No user data or existing caches were deleted. Keep ample free space when reproducing.

This is an ignored test-only workload. It adds no production task, dependency or image cache, changes no format/durability rule, and is not a claimed runtime optimization. The normal release executable's SHA-256 remains identical to the preceding save-state build. After all checks, `cargo clean --profile dev` removed 1.4 GiB of regenerable development artifacts only; that disk cleanup is not a product memory/performance improvement. Background project saving/export and representative device/frame/memory baselines remain unimplemented.

## Raster Conversions

Run `cargo run --release -p parfait-core --example raster_conversion_bench`. Three Apple M1/macOS 15.7.5/arm64/Rust 1.99.1 release runs convert a deterministic 1024×768 active RGBA layer, with twenty iterations per operation/buffer-ownership case. Timers include core validation, conversion and shared-buffer COW, but exclude fixture/reset, independent per-pixel checksums, History, composition, UI/GPU, input and storage. Unique-buffer setup copies outside the timed interval. Every output byte is compared with independent arithmetic; a second conversion verifies no-op Arc/revision preservation. All four case checksums agree across runs.

| Conversion | Shared buffer mean ms range | Unique buffer mean ms range |
| --- | ---: | ---: |
| Brightness to opacity | 0.772460–1.113471 | 0.501106–0.509833 |
| Drawing color | 1.070579–1.371877 | 0.847794–0.944225 |

These explicit whole-layer commands run outside the pen path. Menu guards inspect metadata only. Pixel conversion compares before the first writable access, then uses one mutable suffix; no-op output allocates no pixel buffer. Changed shared pixels still copy the existing full layer, 3,145,728 bytes in this fixture (up to 64 MiB at 4096²). No new full-canvas scratch/cache, dependency or worker is added. This is not a total RSS measurement or an application/pen-latency result. Other desktop apps were running; there is no before/after speedup claim. The unsigned executable is 12,925,952 bytes, 16,624 above the preceding mask-area build; binary size is not RAM.

Isolated native macOS checks execute both Edit commands, select a drawing color, verify exact persisted pixel hashes against independently generated output, Undo twice and Redo twice, reject a mask-target menu click, then normally close/reopen with identical snapshots except the clean-session flag. The original raw fixture remains outside user data. These checks do not establish other-OS, physical-pen or exact external-engine equivalence. Selection-area and non-raster conversion remain unimplemented.

## Mask Area Display

Mask-area display tints the existing checkerboard-composited output in place. It adds two optional layer IDs, no canvas-sized cache/texture, dependency or worker. Uniform-255 coverage bypasses tinting; dense coverage costs one pass over the damaged region. Ordinary pan/zoom/rotation still reuse uploaded pixels. Turning the display on/off or changing its active target refreshes the whole drawing once, including composition: this is the deliberate cost of avoiding an additional untinted image cache. Mask clearing compacts changed coverage to Uniform 255; all-white dense detection occurs only on the explicit command, never on each menu frame.

Run `cargo run --release -p parfait --example mask_area_bench`. Three Apple M1/macOS 15.7.5/arm64/Rust 1.99.1 release runs use a two-layer 1024×768 document and 20 iterations per case. Composition timings include allocation/checksum; tint-only timings exclude source reset copies and checksums. Uniform/Dense independent arithmetic, partial/full output and unchanged raw-content identity are checked before timing. The fixture has an opaque background; existing canvas tests separately cover checkerboard output.

| Coverage | Full tint-only mean ms range | 128×128 tint-only mean ms range |
| --- | --- | --- |
| Uniform 255 | Immediate bypass, near timer resolution | Immediate bypass, near timer resolution |
| Uniform 128 | 0.555208–0.558787 | 0.011206–0.011219 |
| Dense | 1.823060–1.879935 | 0.037983–0.039400 |

The dense fixture's composition alone takes 7.760685–8.047996 ms full / 0.157848–0.173629 ms region. These two-layer values do not replace the existing eight-layer/full-frame measurements or include UI/GPU/input/storage latency. All six tint-case checksums agree across runs, and four existing mask-free clipping checksums remain unchanged in three runs. No physical-pen, RSS, general no-regression or competitive-parity claim follows. The unsigned executable is 12,909,328 bytes, 32 bytes above the preceding editor build; linker size is not a RAM measurement.

## Raster Mask Editing

Mask painting reuses existing dab coverage, spacing, pressure and spray scheduling, but writes a single coverage channel rather than allocating an RGBA proxy document. Uniform coverage becomes Dense only on a changed byte. Per-dab mode has no contribution scratch; capped strokes share the initial mask and lazily allocate 64×64 f32 contribution tiles, bounded by 64 MiB plus 32 KiB of tile pointers at 4096² on a 64-bit target. Existing full-channel COW, History snapshots and compositing allocations remain; these bounds are not total RSS. At most 64 additional 40×40 RGBA mask thumbnails are cached and refresh only after drawing, alongside the existing content previews. No dependency or worker is added.

Run `cargo run --release -p parfait-core --example mask_stroke_bench`. Three Apple M1/macOS 15.7.5/arm64/Rust 1.99.1 runs on a 1024×768 Uniform-127 mask use 64 pointer updates along one line, opacity 0.65, density 0.4 and default fixed spacing. Timings exclude composition/checksum validation, UI/input/GPU, History and storage. Begin timing includes conversion/COW; p95 below measures subsequent updates, including updates that do not cross a dab interval. Checksums agree across runs; every case also verifies raw-content Arc identity, full/region composition and applied-mask equivalence.

| Diameter | Paint/Erase Per-dab p95 ms range | Paint/Erase Capped p95 ms range | Capped scratch bytes |
| --- | --- | --- | --- |
| 32 px | 0.038917–0.045000 | 0.044709–0.045917 | 460,288 |
| 128 px | 0.123459–0.359708 | 0.144291–0.171583 | 460,288 |
| 512 px | 1.775875–1.827125 | 2.156833–2.206125 | 2,098,688 |

These are CPU workload measurements, not physical pen latency or whole-frame budgets. The existing mask-free composition example retains all four output checksums in three new runs; no general speedup/no-regression claim follows. The unsigned executable is 12,909,296 bytes, 33,248 bytes larger than the preceding mask-foundation build. Native isolated macOS testing confirms mask-only paint/erase, five menu actions, cancellation/History coverage in automated tests and exact saved-snapshot equality after normal close/reopen; other OSes, physical tablets and hardware power-loss remain unverified.

## Mask Foundation Check

Absent masks add only optional layer metadata. Uniform masks allocate no coverage canvas; Dense masks share one byte per pixel (at most 16 MiB at 4096²). Logical/History budgets reserve that channel even for Uniform coverage; raw RGBA plus mask reservations remain within the 512 MiB logical budget. These are data-structure bounds, not process RSS measurements. Existing whole-buffer COW remains. No runtime dependency, worker or persistent compositing cache is added.

Three interleaved release `clipping_bench` runs on the same macOS arm64/Rust 1.99.1 configuration retain all output checksums. An initial mask-free clipped candidate regressed to 44.39–44.89 ms; separate const-specialized masked/unmasked chain paths remove that observed regression in the final measurement. The ranges include output allocation/checksum work, not input/UI/GPU/History/autosave. Other desktop workloads were present, so small differences are not a general speedup or no-regression guarantee.

| Existing mask-free workload | Before mean ms | After mean ms |
| --- | ---: | ---: |
| Flat 8-layer 1024×768 | 33.2715–34.5012 | 34.1150–34.6505 |
| Flat 128×128 region | 0.7225–0.7640 | 0.7026–0.7098 |
| Two clipping chains 1024×768 | 37.8552–38.9556 | 38.3927–39.1510 |
| Clipped 128×128 region | 0.7788–0.8262 | 0.7875–0.8774 |

`cargo run --release -p parfait-core --example mask_bench` checks full/region and applied-mask equivalence before timing Uniform 255, Uniform 128 and Dense coverage. The eight-layer Dense fixture measures 38.2377–38.7320 ms full / 0.7757–0.7911 ms region without clipping, and 36.7617–37.4251 / 0.7407–0.7849 ms with Normal-only clipping. This differs from the alternating blend modes above; it is not a before/after speedup. Full refresh remains above 16.7 ms. Physical pen latency and total RAM remain unverified. The unsigned application grows from 12,859,312 to 12,876,048 bytes (+16,736).

For isolated QA, `cargo run --release -p parfait-storage --example mask_fixture -- <new-temporary-directory>` creates a three-layer, two-mask document and expected RGBA output, refusing to replace an existing directory. It is not a user-facing mask editor.

## Row-wise clipping

Clipping eligibility is now filtered once per base into at most 64 borrowed references. A separate flat path avoids the clipped pixel loop. Eligible clipping groups reuse one RGBA row, at most 16 KiB at 4096 pixels wide; flat composition allocates no row scratch. Coverage-255 blending avoids general alpha arithmetic while retaining byte-identical results. This adds neither a full-canvas clip cache, runtime dependency nor background worker. Output allocation and existing whole-layer COW remain.

Three alternating preserved-before/current release runs on Apple M1/macOS 15.7.5/arm64/Rust 1.99.1 use the identical `clipping_bench` workload defined below, including twenty compositions, output allocation and checksum summation. Run two reverses before/after order. No build or UI automation runs alongside the final measurements, but other desktop workloads remain active; this is not an isolated machine.

| Case | Before full mean ms | Current full mean ms | Before 128² mean ms | Current 128² mean ms |
| --- | ---: | ---: | ---: | ---: |
| Eight flat Normal layers | 33.8341–34.8492 | 33.4156–36.3525 | 0.7263–0.7455 | 0.7236–0.9107 |
| Two clipping chains, Normal/Multiply | 152.1848–155.7965 | 38.1087–38.9664 | 3.1213–3.2130 | 0.7882–0.8339 |

Clipped-case median ratios are 4.00× full and 3.87× partial; all output checksums agree. This is a CPU compositor improvement, not a 4× application/pen-latency claim. Full refresh still exceeds 16.7 ms. Earlier diagnostic runs had substantial desktop-load outliers, including 251.2878/103.8868 ms clipped full refresh and a 71.2179 ms flat refresh before the separate flat path was finalized; those observations are retained rather than treated as stable speed limits.

Three before/after runs of the existing blending/wash workloads, unlocked and alpha-locked, retain damage/scratch metadata in all 144 paired cases. General brush speedup is not established: unlocked 512 px Smudge p95 ranges are 10.2938–10.4639 before and 10.3574–10.6756 ms current, while wash Paint Per dab ranges are 5.2817–5.3498 and 5.3695–5.7048 ms. Some cases increase, and neither the prior Blur regression nor whole-layer COW is resolved. These examples do not report brush pixel checksums; pixel correctness is covered separately by reference/brush tests and native framebuffer comparison.

The complete release executable remains 12,826,080 bytes. This is not a RAM measurement. Color-managed/grouped layers, full application frame tails, physical tablet latency and other operating systems remain unverified.

## Initial raster clipping milestone

The initial pixel-first clipping implementation added one serialized flag per layer, no dependency/background task and no group-sized pixel cache. It allocated only the requested output region, using a four-byte stack color per pixel; the newer row-wise implementation above adds bounded row scratch instead. Real painting still incurs existing whole-layer COW, and scene changes can require a full refresh.

Run `cargo run --release -p parfait-core --example clipping_bench`. Two Apple M1/macOS 15.7.5/arm64/Rust 1.99.1 thin-LTO runs on 2026-10-19 use deterministic 1024×768 RGBA in eight layers, opacity 0.7. The clipped case uses two bases with three clips each and alternating Normal/Multiply; the flat case uses Normal. Each case checks region/full pixel equality before timing twenty compositions. Output allocation and checksum summation are included; fixture creation, drawing, History/COW, storage, UI/input/GPU and checkerboard are excluded.

| Case | Full 1024×768 mean ms | Partial 128×128 mean ms |
| --- | ---: | ---: |
| Eight flat Normal layers | 33.6433–33.7356 | 0.7437–0.7599 |
| Two clipping chains, Normal/Multiply | 152.5789–153.3450 | 3.1411–3.1704 |

Checksums agree across runs. Output allocations are 3,145,728 and 65,536 bytes respectively, with no additional full-canvas clip buffer. These are different workloads, not a controlled old/new speed comparison. Full refresh is expensive and does not meet a 16.7 ms frame budget here; retain bounded damage composition and profile large-chain/full-refresh costs. No p95, total RSS, physical pen latency or competitive parity is established. The unsigned release executable is 12,826,080 bytes (+16,544 over the menu-catalog milestone); this is not a memory measurement.

## Static menu catalog

The expanded menu-outline milestone used immutable descriptor slices and one renderer for actions, toggles, planned leaves and submenus. No whole-catalog parsing/cloning, persistent placeholder-command registry, pixel buffers, background work or runtime dependencies were added by that outline. Only opened branches render their descriptors. Each level has a bounded scrolling viewport; the popup's default sizing is explicitly expanded to the allowed height so short menus need not scroll unnecessarily. That milestone retained preferences v13, artwork v3 and portable brushes v8; clipping introduced preferences v14 and artwork v4. Layer organization now uses preferences v15 without changing artwork v4 or portable brushes v8.

The layer-organization increment adds no dependency, pixel-processing algorithm, persistent image cache or background worker. Active-only moves reuse the existing at-most-64-layer metadata reorder path and shared pixel buffers. Navigation reuses selection's revision-tag synchronization; a synthetic frame regression checks that it does not upload the canvas texture. The rename modal retains at most 128 Unicode characters and one shared buffer reference, releasing it on confirm/cancel/stale-target rejection. On the same macOS arm64 release configuration, the unsigned executable grows from 12,826,080 to 12,859,312 bytes (+33,232). This is binary size, not measured working-set memory or pen latency; no speedup is claimed for these metadata/UI commands.

The 2026-10-19 arm64 release executable is 12,809,536 bytes, 16,112 fewer than the preceding alpha-lock build after consolidating menu closures. This artifact-size observation does not establish a CPU/RAM or pen-latency improvement. Raster/compositing/storage code is unchanged in this increment, and their existing regression tests remain required.

## Stable IDs and alpha lock

The 2026-10-19 arm64 release executable is 12,825,648 bytes (+33,136 over the top-menu build). IDs, alpha flags and bounded session metadata add no runtime dependency, image-sized cache or background task. Selection reuses at most 64 IDs alongside its mask. Raster writes acquire a writable shared buffer only after an actual change; subsequent writes edit the unique buffer directly. Transparent/same-color locked strokes preserve buffer identity and allocate no contribution tiles. This avoids no-op COW, not the whole-layer COW needed for real edits.

Three alternating preserved-before/current-unlocked/current-alpha-locked release runs on Apple M1/macOS 15.7.5/arm64/Rust 1.99.1, without concurrent compilation or UI actions, produced these 512 px p95 CPU-update ranges. Workload definitions and exclusions below still apply.

| Case | Before ms | Current unlocked ms | Alpha locked ms |
| --- | ---: | ---: | ---: |
| Paint, blending fixture | 4.5403–4.8582 | 4.6044–4.6569 | 4.2808–4.4475 |
| Blend | 5.7213–6.1860 | 5.7922–5.8093 | 5.9455–6.1421 |
| Blur | 6.3572–6.4905 | 7.0470–7.5572 | 6.8741–7.3104 |
| Smudge | 10.0469–10.1965 | 9.6954–10.2519 | 9.9426–9.9842 |
| Paint, COW Per dab | 4.6091–4.6227 | 4.6647–4.8621 | 4.2787–4.4054 |
| Paint, COW Per stroke | 5.6135–5.6232 | 5.8258–6.0529 | 5.3958–5.4344 |
| Erase, COW Per dab | 3.8237–3.8368 | 3.8689–3.9482 | 4.2717–4.3371 |
| Erase, COW Per stroke | 6.2561–6.2789 | 6.3210–6.4120 | 5.4078–5.4466 |

Safety checks are not free: Blur retains a measurable regression in this workload and requires further profiling. The alpha-locked Blur maximum is 13.0097 ms despite its lower p95; averages must not hide spikes. Locked Erase performs RGB painting rather than alpha removal, so it is not the same work as unlocked Erase. Unlocked blending damage-byte counts and scratch capacities match the preserved executable for all 12 cases. Run `blending_bench` or `wash_bench` with `--alpha-lock` for the additional variant. These are not speedup, total RSS, GUI/GPU or physical pen-latency measurements.

## Top-menu shell

The localized eight-menu shell reuses typed command dispatch and cached key hints; it adds no pixel buffers, serialized state, runtime dependencies or background tasks. The 2026-10-19 arm64 release executable is 12,792,512 bytes, 49,520 fewer than the preceding layer-row build after reorganizing the menu closures. This is an artifact-size observation, not a CPU/RAM or input-latency improvement claim. The raster/compositing/storage algorithms are unchanged, and the complete regression suite still passes.

## Layer rows and raster combining

The 2026-10-19 arm64 release executable is 12,842,032 bytes (+49,744 over the preceding layer-command build). No dependency or background service is added. Selection is one 64-bit set; drag targets/order metadata are bounded by 64 layers, and reordering moves shared buffers. Edge-scroll repaint stops at its limits. Explicit combines allocate one canvas-sized RGBA output (up to 64 MiB); transfer may additionally COW its cleared source (up to another 64 MiB). History/autosave sharing and whole-layer COW still determine higher peak memory; this is not a process-RSS bound.

A single preserved-before/rebuilt-after release `blending_bench` run retains identical damage-byte counts and scratch capacities for all 12 cases. At 512 px, p95 CPU update times before/after are Paint 3.9893/4.5040 ms, Blend 5.6256/5.7221, Blur 6.7909/6.3583 and Smudge 10.3350/10.0647. Differences include noise and mixed regressions/improvements, not a speedup claim. The compositor now supports a bounded layer-selection mask; full/partial pixel-equivalence regressions remain green. GUI/GPU work, physical pen latency and large-document peaks are not measured by this workload.

## Layer-command footprint

Layer menu, palette and keyboard actions share static disabled-reason queries and transaction setters. Moving/flagging a layer preserves its `Arc` pixel buffers; rendering command controls does not clone a document or serialize settings. Toggle menus reuse cached shortcut hints instead of building space-padded label/key strings. No runtime dependency, periodic task, raster/compositing algorithm or save durability change is introduced. The 2026-10-19 arm64 release executable is 12,792,288 bytes versus 12,792,240 at the preceding reliability milestone (+48 bytes with this build's linker layout). This is an artifact size, not a memory or speed measurement. No new pen-latency claim is made; existing whole-layer COW and large-document peaks remain.

## Transaction and no-op reliability workload

On 2026-10-19, named History replaced eager redo-clearing checkpoints with shared-buffer begin/commit/cancel transactions. Rendering its at-most-31 rows borrows metadata; no per-frame pixel comparison or snapshot cloning is introduced. Per-dab Paint/Erase now compares the computed RGBA result before acquiring a writable buffer and reports damage only after a byte changes, matching the existing capped/blending policy. Zero-strength, empty-mask, identical opaque ink and transparent erasing keep pixel identity and revision, avoiding no-op COW and preserving redo. Default accumulation pixel output remains covered by core/partial-composition regressions.

Three alternating before/after release `wash_bench` runs used the same Apple M1/macOS 15.7.5/arm64/Rust 1.99.1 environment and the populated, history-like COW fixture described below, without concurrent UI input or compilation. The old executable was preserved before rebuilding. Ranges are p95 movement-update CPU milliseconds, not physical pen latency or a controlled application-level speedup:

| Per-dab case | Before p95 ms | After p95 ms |
| --- | ---: | ---: |
| Paint 24 px | 0.0218–0.0248 | 0.0245–0.0249 |
| Erase 24 px | 0.0149–0.0213 | 0.0207–0.0210 |
| Paint 128 px | 0.2840–0.3585 | 0.3310–0.3438 |
| Erase 128 px | 0.2244–0.2712 | 0.2777–0.7549 |
| Paint 512 px | 4.1838–4.7198 | 4.6848–4.8079 |
| Erase 512 px | 3.4437–3.8948 | 3.4589–4.0400 |

Actual-write comparison adds work; these measurements do not establish a speed improvement. Timing noise is visible even in unchanged capped paths; the 128 px Erase outlier must not be hidden. Per-dab scratch stays zero, capped scratch capacities are unchanged, and large brushes/full-layer COW still need profiling. The stripped release executable is 12,792,240 bytes, +33,248 over the previous brush milestone; this is not RAM usage. No runtime dependency, background timer, snapshot-format increase or durability relaxation was added.

Layer growth/save preflight is checked metadata arithmetic proportional to layer count, not serialization or pixel allocation. The 512 MiB snapshot budget includes metadata/header and is distinct from logical pixels; neither bounds peak RSS across history, autosave and rendering. The conservative 128 MiB history policy retains at least one edit even when that minimum exceeds its nominal budget.

## Changed path

Previously, every painted frame composited all pixels of every visible layer, applied a checkerboard to the entire canvas and uploaded a complete texture. The canvas now accumulates dab bounds for each input batch, composites only that rectangle and submits a partial texture update. Undo, layer edits and opening another document still request a full refresh. Bounds are transient; the project format is unchanged.

Opaque pixels take a fast blend path. Picking a color composites one pixel. Layer thumbnails catch up after the stroke; the canvas and navigator stay live. Preferences serialization/writes are deferred during an active stroke. Artwork autosave still requests a background snapshot every 400 ms and immediately after a completed stroke.

## Reproduce

```sh
cargo run --release -p parfait-core --example stroke_bench
cargo test --workspace
```

The benchmark performs 240 identical incremental updates with a 24 px pressure-varying round brush, comparing full and damaged-region compositing in the same executable. It paints one layer; other layers are blank except for opaque paper. Initial allocation/full rendering is outside the timer. It includes dabs, allocation and CPU layer composition; it **excludes** GUI/checkerboard conversion, GPU upload, input hardware, history snapshots and autosave. It does not measure end-to-end application latency.

One Windows x86-64 GNU release run on an Intel Core i7-8700 @ 3.20 GHz development machine (Rust 1.99.1, thin LTO, 2026-10-18):

| Canvas / layers | Full composition, ms/update | Partial composition, ms/update | Full / partial RGBA bytes per update |
| --- | ---: | ---: | ---: |
| 1024 × 768 / 2 | 3.486 | 0.017 | 3,145,728 / 1,769 |
| 2048 × 2048 / 8 | 51.207 | 0.020 | 16,777,216 / 1,769 |

These are workload-specific averages, not p95 latency, a fixed speedup for all drawings, or a comparison against another application. The full path here includes the new opaque-pixel fast path, so this is a path comparison rather than a checkout-to-checkout benchmark. Wide strokes or input spanning a large rectangle reduce the benefit.

## Correctness and remaining costs

Tests compare incremental pixels to full composition for every built-in tip, image tips, erasing, pressure, layer opacity and clipped/outside strokes. A headless application test reconstructs the texture from actual full/partial upload commands, checks checkerboard alignment and verifies full invalidation after a layer edit and undo. Existing forced-process-exit recovery tests remain required.

Whole-layer copy-on-write buffers can still cause a full layer copy after history checkpoints or autosave snapshots. Full-document serialization/compression happens on the background worker but consumes CPU/memory bandwidth. Settings persistence after pen-up can still be expensive when a large material library actually changes. Unchanged preferences now skip cloning/serialization through a typed saved-state comparison, described below. Tiled snapshots, asynchronous changed-preference writes and real-device p95/p99 measurements remain follow-up work; current measurements do not establish the product's overall lightweight target.

## Blend-mode workload

Run `cargo run --release -p parfait-core --example blend_bench` for 30 compositions of a 1024 × 768 document with eight populated translucent layers (pixel alpha 180/255, layer opacity 0.8). It compares the full canvas with a 32 × 32 damage rectangle. Allocation and CPU blending are included; strokes, UI, GPU, history and autosave are excluded. This is deliberately heavier than the mostly blank-layer stroke benchmark above.

One Windows development-machine release run (same CPU/toolchain as above, 2026-10-18):

| Mode | Full canvas, ms/update | 32 × 32 region, ms/update |
| --- | ---: | ---: |
| Normal | 170.661 | 0.188 |
| Multiply | 211.501 | 0.444 |
| Soft light | 329.872 | 0.324 |

These single-run averages are sensitive to scheduling and are not pen latency measurements. Complex modes cost more, and full refreshes on heavily populated documents remain a performance limitation. Painting continues to use damage regions.

## Spatial Smudge workload

`blending_bench` now also measures Smudge and reports first-dab, p95 and maximum CPU input-update times, in addition to the average. The same 120-update fixture, default 12%-diameter spacing and exclusions described below apply. A subsequent Apple M1/macOS 15.7.5/arm64/Rust 1.99.1 thin-LTO run on 2026-10-19 gave:

| Smudge diameter | Mean ms/update | First pickup ms | p95 ms/update | Max ms/update | Scratch bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| 24 px | 0.0809 | 0.0263 | 0.1195 | 0.1279 | 12,544 |
| 128 px | 0.1857 | 0.2097 | 0.6984 | 1.0208 | 278,784 |
| 512 px | 0.7050 | 3.0788 | 10.1144 | 10.2476 | 4,260,096 |

Two bounded premultiplied patches are allocated once per Smudge stroke and reused. Same-radius movement reuses previous cells directly instead of resampling an identity mapping. Other operations do not allocate these patches. Smudge never schedules stationary-spray redraws. This is not an application-frame or physical-latency measurement: large brushes have infrequent expensive dabs, so the mean conceals spikes. At 512 px, a single measured update already costs around 10 ms before GUI/GPU/COW/saving; denser spacing, rapid long movement and simultaneous saves can be substantially more expensive. Small default tips do not establish large-canvas parity with another editor. Whole-layer copy-on-write and history costs remain follow-up work.

## Smooth response follow-up

After shared cubic pressure interpolation and blend-mode integration, the same `stroke_bench` command on the Windows development machine above produced these single-run averages (2026-10-18):

| Canvas / layers | Full composition, ms/update | Partial composition, ms/update |
| --- | ---: | ---: |
| 1024 × 768 / 2 | 9.604 | 0.019 |
| 2048 × 2048 / 8 | 177.567 | 0.032 |

This measures the combined rendering path at that revision, not isolated spline overhead or a controlled before/after comparison. Full composition remains expensive and deserves separate profiling; stroke damage updates remain local. Response evaluation does not allocate per sample. GUI, GPU, autosave and device latency remain excluded.

## Variable response point workload

The brush now owns 2–16 response coordinates and samples them without creating temporary points. Run `cargo run --release -p parfait-core --example stroke_bench`, then the same command with `-- --dense-curve` for 16 non-uniform input points and a square-root output response. One macOS arm64 Apple M1 run using Rust 1.99.1 on 2026-10-19 produced these single-run averages:

| Points | Canvas / layers | Full, ms/update | Partial, ms/update | Partial bytes/update |
| --- | --- | ---: | ---: | ---: |
| 2 | 1024 × 768 / 2 | 4.034 | 0.008 | 1,769 |
| 2 | 2048 × 2048 / 8 | 76.905 | 0.013 | 1,769 |
| 16 | 1024 × 768 / 2 | 3.657 | 0.009 | 2,198 |
| 16 | 2048 × 2048 / 8 | 73.228 | 0.017 | 2,198 |

The dense curve changes painted diameter and damaged-region size, so these are representative workloads, not an isolated point-count overhead measurement. Different full-path timings are not evidence that more points improve performance. GUI, GPU, history, saving and physical input latency remain excluded. Pixel-equivalence tests also exercise arbitrary-input falling curves.

For explicit Bezier evaluation, add `--bezier` (for example `-- --dense-curve --bezier`). It initializes equivalent manual controls on the same response, using a bounded 24-step input solve per sample without heap allocation. A single Apple M1/macOS arm64/Rust 1.99.1 release run on 2026-10-19 with 16 points produced 3.874/0.010 ms per full/partial update at 1024 × 768 with 2 layers and 71.522/0.017 ms at 2048 × 2048 with 8 layers; partial upload payload was 2,198 bytes/update. These whole-workload averages do not isolate curve-evaluation cost or establish pen latency or a speedup. Region/full pixel-equivalence tests include manual handles.

## Gap and timed-spray workload

Run `cargo run --release -p parfait-core --example stroke_bench -- --gap-narrow --spray` to use the Narrow interval and add 1/60 s of stationary accumulation at the current position on each movement update. This uses a 24 px round brush, identity pressure curve and 30 extra dabs/second in the same 240-update workload. The time function performs no allocation or filesystem access, caps one elapsed update at 100 ms and emits at most 12 additional dabs. This bounds catch-up work, not total brush cost: 512 px tips at high rates and frequent copy-on-write snapshots remain expensive.

An Apple M1/macOS arm64/Rust 1.99.1 release run on 2026-10-19 produced these single-run averages:

| Canvas / layers | Full, ms/update | Partial, ms/update | Partial bytes/update |
| --- | ---: | ---: | ---: |
| 1024 × 768 / 2 | 3.739 | 0.014 | 2,703 |
| 2048 × 2048 / 8 | 71.016 | 0.024 | 2,703 |

This includes CPU dabs/compositing but excludes GUI, GPU upload, saving, input delivery and actual frame scheduling. It is not a measured stationary pen latency, application frame budget or a cross-product comparison. Partial/full pixel equivalence is separately tested with timed dabs across every tip and erasing mode.

## Lightweight maintenance audit

### Display and font follow-up

Display-only flips, rotation, zoom, ruler visibility, background and cursor changes reuse the canvas texture. Regression tests assert no artwork revision/history edit or canvas texture upload; pixel-grid painting at 8×+ uses visible document-coordinate lines with a 4096-line cap, not a pixel-sized overlay buffer. Keyboard dispatch collects only pressed-key metadata instead of cloning unrelated pointer/text payloads. Fast zoom uses each key event's modifiers, so a coalesced key/modifier release cannot discard the command. These are bounded-work/correctness checks, not measured end-to-end latency gains.

No Cargo manifest or lockfile dependency changed. One unmodified 1,574,352-byte Pretendard Regular face replaces runtime system-CJK font discovery; fallback and icon fonts remain. On Apple M1/macOS 15.7.5/arm64/Rust 1.99.1, the stripped thin-LTO executable grew from 11,024,800 bytes (pre-font/pre-display Smudge build) to 12,659,568 bytes. This includes both the font and new UI code/alignment, not only the font. Executable size is not resident or private memory; retaining fallbacks is intentional for glyph safety.

The v7 unchanged-preference workload below still returns before serialization/database access. Three separate 20-poll release-test runs with 64 shared-mask presets and a 16,818,585-byte saved JSON record measured 0.003998, 0.017042 and 0.002015 ms/poll. Scheduling/cache noise matters at these durations; these are not a new controlled before/after comparison. Changed large-library saves still serialize the complete record synchronously while not drawing. Whole-layer COW, populated-layer refreshes and save/undo spikes remain important optimization targets; no durability, limits or validation were removed.

A single warmed native release idle sample on the same machine used a 512 × 384 two-layer fixture, a 1440 × 920 logical/2× viewport and the display-settings window open, with no input during the sample. Over 10 seconds, `ps` process CPU time advanced from 1.46 to 1.49 seconds and RSS from 35,088 to 35,712 KiB. This is a coarse small-document baseline, not a controlled improvement, private-memory/GPU footprint, large-document limit, battery measurement or SAI comparison. Shared/compressed-page accounting and other system activity affect RSS; repeat under representative drawing/save workloads before making memory or latency claims.

Image-tip masks and document pixels already use shared `Arc` buffers. No new runtime dependencies or removal of accessibility, recovery, validation or image formats were needed for this audit. Build caches are not distribution size or runtime memory.

Unchanged settings previously cloned and serialized the complete preference record every maintenance interval, then compared two JSON strings. The application now compares typed persisted fields first and returns before cloning, JSON encoding or database access. The last successfully saved typed snapshot shares image masks with the live library; the complete saved JSON string is no longer kept resident. Runtime-only workspace/brush-library caches are excluded. Validation and the existing synchronous-FULL database write still run on actual changes, and only successful writes advance the cache. Older records upgrade to the supported version (7 at this audit, currently 9); failed/future-version loads remain protected.

Reproduce the opt-in, non-shipping benchmark:

```sh
cargo test --release -p parfait preferences::tests::unchanged_material_library_poll_benchmark -- --ignored --exact --nocapture --test-threads=1
```

The fixture has 64 presets sharing a 256 × 256 image mask, a 16,810,960-byte JSON record, an initial save outside the timer and 20 forced unchanged-maintenance calls. Three separate runs per executable on Apple M1/macOS 15.7.5/arm64/Rust 1.99.1 with thin LTO on 2026-10-19 gave:

| Workload | Before, ms/call | After, ms/call |
| --- | ---: | ---: |
| Unchanged material-library maintenance | 22.322–25.089 | 0.000981–0.002252 |

These measure only the unchanged-state fast path, not changed-library saving, normal per-frame cost, GPU work, physical pen latency or total application memory. A new typed snapshot has a small bounded metadata cost; image payloads remain shared. OS memory-accounting samples are not evidence of an equivalent net memory reduction or parity with another painting application.

Idle root-window maintenance requests changed from 100 ms to 1 second. Pending saves, an active stroke and pending close retain 100 ms maintenance, and continuous spraying retains its independent shorter interval. Native input and framework animations can request frames immediately. Artwork autosave's 400 ms submission interval, acknowledgments, final flush and recovery durability are unchanged. This reduces scheduled idle polling, not a measured 90% reduction in CPU or power.

Keep release packaging, idle CPU/working-set measurements, populated-layer workloads, large-library edits and save/undo spikes in future performance gates. Do not trade away safe saves or add unbounded caches to improve a small benchmark.

## Local blending workload

Run `cargo run --release -p parfait-core --example blending_bench`. A 1024 × 768 two-layer document has a deterministically populated translucent active layer. Each case performs 120 four-pixel movement updates with a round brush, default 12%-diameter spacing, pressure disabled, Flow 0.5 and manual blur radius 32. The timer includes stroke/scratch construction, dabs and partial CPU composition/allocation. Fixture construction, GUI/checkerboard/GPU, input delivery, history/COW snapshots and saving are excluded. These are per-input-update averages, not per-dab times: larger diameters stamp less frequently at this spacing, and some updates have no damage.

One Apple M1/macOS 15.7.5/arm64/Rust 1.99.1 thin-LTO release run on 2026-10-19:

| Diameter | Paint ms/update | Blend ms/update | Blur ms/update | Blur scratch bytes |
| --- | ---: | ---: | ---: | ---: |
| 24 px | 0.0302 | 0.0411 | 0.0466 | 54,464 |
| 128 px | 0.0828 | 0.1264 | 0.1607 | 360,640 |
| 512 px | 0.3314 | 0.4209 | 0.5417 | 3,739,840 |

Default per-dab Paint/Erase and Blend add no scratch buffers. Blur reserves scratch once per stroke and reuses it; the maximum validated capacity is tested. Existing copy-on-write can still copy a complete active layer after history/autosave snapshots. Workload averages hide costly large dabs and do not establish frame-time tails, physical pen latency, total memory or parity with another editor. Narrow gaps and timed spraying increase actual dab work. Slow-reference alpha/filter tests and partial/full equivalence are correctness gates independent of these timings.

## Pressure spacing workload

Run `cargo run --release -p parfait-core --example spacing_bench`. On the same Apple M1/macOS 15.7.5/arm64/Rust 1.99.1 thin-LTO environment, three runs on 2026-10-19 use a 1024 × 768 two-layer document, round Paint tip, diameter pressure disabled, Flow 0.1 and 120 four-pixel horizontal updates at constant input pressure 0.25. Construction/dabs/partial CPU composition are timed; fixture creation, UI/GPU/input, history/COW snapshots and saving are excluded. Enabled spacing uses default 12% spacing, a linear curve and 10% minimum: intervals become 32.5% of their disabled value, intentionally doing about three times as many dabs.

| Diameter | Disabled mean ms/update | Enabled mean ms/update | Enabled p95 ms/update |
| --- | ---: | ---: | ---: |
| 24 px | 0.0183–0.0195 | 0.0441–0.0552 | 0.0510–0.1120 |
| 128 px | 0.0765–0.1006 | 0.2235–0.2369 | 0.3082–0.3437 |
| 512 px | 0.2781–0.3111 | 0.8409–0.8915 | 4.2385–4.5083 |

Paint scratch remains zero bytes. The independent curve adds bounded per-brush/per-stroke metadata (2–16 anchors and optional handles), not another pixel buffer or per-dab heap allocation. No runtime dependency or periodic task is added. The complete stripped release executable grows from 12,659,568 to 12,758,976 bytes (+99,408), including shortcut editing/dispatch and this brush feature together; bundle signing bytes are excluded. This does not measure RAM or establish competitive parity.

A single before/after `blending_bench` check with pressure spacing disabled retains identical damage-byte counts and scratch capacities. Its 512 px Smudge p95 is 10.3846/10.7968 ms and maximum 13.2255/16.4228 ms. Runs are not controlled enough to attribute timing differences; they show that expensive large brushes still need profiling, not a speedup claim. Pressure spacing at the 0.5 px floor with large blending/blur/smudge tips can do substantially more work than this Paint fixture. Full application frame/physical pen latency and large-document COW remain unverified.

## Capped stroke opacity workload

Run `cargo run --release -p parfait-core --example wash_bench`. Three Apple M1/macOS 15.7.5/arm64/Rust 1.99.1 thin-LTO runs on 2026-10-19 use a 1024 × 768 two-layer document with a deterministically populated active layer at alpha 180. Both accumulation modes retain a document snapshot to impose history-like whole-layer COW. Opacity is 0.4, density (`flow`) 0.25, pressure disabled and spacing 12% of diameter. The mean includes construction/first dab and 120 four-pixel movement updates with a small sinusoidal Y offset, plus damaged-region CPU composition/allocation. The p95/max columns measure movement updates separately. Fixture preparation, UI/GPU/input delivery and saving are excluded. Larger tips produce fewer dabs at this spacing; these are not per-dab or frame timings.

| Capped case | Mean ms/update | p95 ms/update | Maximum ms/update across runs | Scratch bytes |
| --- | ---: | ---: | ---: | ---: |
| Paint 24 px | 0.0283–0.0439 | 0.0350–0.0564 | 0.1058 | 214,528 |
| Erase 24 px | 0.0276–0.0306 | 0.0354–0.0512 | 0.0742 | 214,528 |
| Paint 128 px | 0.1041–0.1249 | 0.3738–0.4522 | 0.5175 | 493,056 |
| Erase 128 px | 0.1110–0.1135 | 0.3960–0.4255 | 0.5217 | 493,056 |
| Paint 512 px | 0.3968–0.4091 | 5.2184–5.3164 | 5.6157 | 2,196,992 |
| Erase 512 px | 0.4429–0.4434 | 5.8081–5.8228 | 6.0347 | 2,196,992 |

Capped mode is optional and costs extra work: the same 512 px per-dab cases average 0.3158–0.3256 ms for Paint and 0.2510–0.2558 ms for Erase, with zero contribution scratch. Capped first-dab times at 512 px are 3.6784–4.0105 ms for Paint and 4.2478–4.3152 ms for Erase, including the imposed COW. These small fixtures do not bound large-document COW, actual frame time or physical pen latency.

Contribution storage is lazy 64²-cell tiles, not a full extra RGBA scratch image. Fully covering the largest supported 4096² document uses at most 64 MiB payload plus 32 KiB indexing on a 64-bit target, excluding allocator overhead, original/current layer pixels and history/save snapshots. Transparent erase and painting identical opaque ink allocate neither tiles nor destination COW and do not dirty the document. Baseline buffers remain shared until a real pixel change; whole-layer COW is still a follow-up bottleneck. The complete stripped release executable is 12,758,992 bytes, not a RAM measurement. No manifest/lockfile dependency or background task was added, and durability/format validation were not weakened.
