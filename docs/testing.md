# Testing

Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`.

The Windows GitHub Actions configuration is provided as [ci-template.yml](ci-template.yml). CI is not enabled yet: publishing the workflow requires GitHub credentials with workflow permission. To enable it, place the template at `.github/workflows/ci.yml` and push using credentials with that permission.

Core tests cover alpha compositing, visibility, copy-on-write snapshots, continuous strokes, erasing, locked layers and undo/redo branching. Canvas tests cover rotated coordinate inversion and cursor-anchored zoom.

Headless egui input tests verify that hand-tool dragging pans without changing pixels, switching back to the pen draws, and a hue chosen while the current color is black is retained when saturation/value is raised. These tests supplement rendered-frame inspection; they do not replace native OS input or physical tablet validation.

Storage tests round-trip snapshots, corrupt the newest payload and check fallback, and verify exclusive recovery ownership. Integration tests launch `recovery_probe` as a separate process, wait for a durable fixture milestone, forcibly kill it, and reopen the database:

1. Kill during an uncommitted replacement transaction: the prior brush pixels remain.
2. Kill after a committed layer edit: the latest layer count and brush pixels return.

These tests deliberately bypass normal shutdown. They establish process-crash recovery, not hardware power-loss guarantees.

For visual QA, `parfait --data-dir <temporary-directory> --screenshot <output.png>` exports the application's own rendered framebuffer after its first few frames. The app stays open for interaction. This option is inactive during normal startup.

Manual checks use `parfait --data-dir <temporary-directory>`: draw, switch layers, undo/redo, pan/zoom/rotate, save/export, wait for Saved locally, kill the app process, relaunch with the same directory, and compare pixels and layer state. Verify that a second app using that directory is rejected and the original continues working. Hardware pressure/tilt must be checked with real tablets before claiming device support.
