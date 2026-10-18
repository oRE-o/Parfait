# Parfait

A small, open-source drawing app in Rust. A quiet canvas, familiar pen-first controls, layers, and automatic local recovery.

**Early desktop prototype (0.1).** The first version prioritizes a complete draw → save → restart workflow over a large tool catalog.

![Parfait drawing workspace](docs/images/workspace.png)

## Platform direction

Windows, macOS and Linux are first-class desktop targets. Mobile apps are planned after the desktop foundation, with Android and iOS/iPadOS as intended targets. The Rust drawing core and project format should be shared across platforms; input, lifecycle, file access and UI layout must adapt to each device.

The current prototype has been built and tested on Windows only. macOS, Linux and mobile support are goals, not verified release claims. See the [platform plan](docs/platforms.md) for status and acceptance criteria.

## Try it

Install a current stable Rust toolchain and the platform build tools, then:

```sh
cargo run -p parfait
```

On Windows, the standard MSVC toolchain requires Visual Studio C++ Build Tools. The GNU toolchain also works with MinGW on PATH. The app uses native egui/eframe and OpenGL; no browser runtime is required.

```sh
cargo build --release -p parfait
cargo test --workspace
```

Run an isolated session with `parfait --data-dir PATH`. Otherwise recovery data lives in `%LOCALAPPDATA%/Parfait` on Windows, `$XDG_DATA_HOME/Parfait` or `~/.local/share/Parfait` on other desktops. Only one app can write a given recovery folder at a time.

## In this version

- Round and soft brushes, eraser, size and opacity, color picker and palette.
- Import PNG brush tips: alpha masks for transparent PNGs; inverted luminance for opaque PNGs.
- Raster layers: add, duplicate, remove, reorder, rename, show/hide, lock, and opacity.
- Bounded undo/redo with copy-on-write pixel snapshots.
- Pan, cursor-centered zoom, view rotation, fit view and navigator preview.
- Local `.parfait` projects, PNG export, background autosave and startup recovery.
- Dark workspace: tools on the left, canvas in the middle, navigator and layers on the right.
- Compact desktop palettes: icon tool rail, brush previews and size presets, hue/SV color wheel, canvas rulers and live layer thumbnails.

## Controls

| Action | Shortcut |
| --- | --- |
| Brush / eraser | B or P / E |
| Brush size | [ / ] |
| Move canvas | Space + drag, or middle-button drag |
| Rotate view | Shift + Space + drag |
| Zoom view | Ctrl + Space + drag, or wheel |
| Pick canvas color | Alt + click |
| Fit and reset rotation | Ctrl + 0 |
| Undo / redo | Ctrl + Z / Ctrl + Y or Ctrl + Shift + Z |
| Add layer | Ctrl + Shift + N |
| New / open | Ctrl + N / Ctrl + O |
| Save / save as | Ctrl + S / Ctrl + Shift + S |
| Export PNG | Ctrl + Shift + E |
| Hide side panels / help | Tab / F1 |

Keyboard shortcuts pause while editing a text field. View transformations do not alter image pixels.

## Recovery contract

An unnamed drawing is protected from the first edit. Completed strokes request an immediate background save; longer edits request a snapshot approximately every 400 ms. Pending work is coalesced so slow disks cannot create an unlimited queue.

The UI shows **Saved locally** only after the worker confirms a committed SQLite transaction. Three checksum-verified compressed snapshots are retained. An invalid newest payload falls back to an earlier valid generation, and the app reports that fallback. An unrecoverable database produces an error rather than silently starting over.

An unexpected exit restores the latest valid committed snapshot on next launch. Input not yet committed may be lost. Disk failure and real power-loss durability are not guaranteed by the process-kill tests. Keep exported project backups for important work.

Before New/Open replaces the current drawing, the app creates `before-new.parfait` / `before-open.parfait` in its recovery directory. Those backups are single rotating slots. Save a named project to keep an independent copy. Undo history is session-local; pixel and layer state survives restart.

## Development

See [architecture](docs/architecture.md), [testing](docs/testing.md), and [roadmap](docs/roadmap.md).

```text
apps/parfait/             Native application, panels, input and canvas view
crates/parfait-core/      Raster document, brush engine, undo/redo
crates/parfait-storage/   Versioned SQLite snapshots, worker, recovery tests
docs/                    Architecture, testing, roadmap
```

## Current limits

The prototype starts at 1024 × 768 and uses whole-layer CPU buffers and CPU compositing. Large-canvas tiled rendering is planned. Pressure is used only when delivered by the platform input backend; physical pen pressure/tilt and device-specific behavior still need hardware validation. Imported tip selection is session-local. ABR/SUT import, advanced mixing, selection, transforms, and liquify are not implemented. Linux/macOS source portability has not yet been validated on hardware.

## License

MIT. See [LICENSE](LICENSE). Contributions should include source provenance, tests for behavior changes, and no third-party assets without redistribution permission.

Toolbar icons use bundled Font Awesome Free 6.7.2 (SIL OFL 1.1 for the font). See [asset attribution](apps/parfait/assets/fontawesome/README.md) and Help → Third-party notices in the app.
