# Roadmap

## 0.1

Basic drawing, layers, PNG tips, local projects and verified process-crash recovery. Prioritize a usable end-to-end workflow.

## Next

- Establish Windows, macOS and Linux build/test coverage and native packaging; validate each on real hardware before claiming support.
- Configurable canvas dimensions and keyboard bindings.
- Typed edit commands, layer thumbnails, and grouped property edits.
- Persistent brush library, tags and import compatibility reports.
- Hardware-tested pen sampling, pressure curves and stabilization.
- Tiled buffers, partial texture updates and GPU layer compositing.
- Tests for disk exhaustion, permission failures and storage migration.
- Selection, clipping and masks, then geometric transforms and liquify.
- Additional brush formats based on versioned fixtures and documented mappings.

Each feature needs observable acceptance criteria. A parsed brush file is not evidence of matching brush behavior; a successful save call is not a substitute for a recovery test.

## Mobile apps (later)

- Target Android and iOS/iPadOS after the desktop foundation; device priorities and release dates remain undecided.
- Reuse the Rust drawing core and versioned project format, with an adaptive touch/stylus workspace.
- Separate platform file access, lifecycle, input and rendering before choosing the mobile shell.
- Validate recovery after background suspension and OS termination, memory pressure, sandboxed import/export and tablet pen behavior.

See the [platform plan](platforms.md) for the support matrix and release criteria.
