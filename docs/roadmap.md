# Roadmap

## 0.1

Basic drawing, layers, PNG tips, local projects and verified process-crash recovery. Prioritize a usable end-to-end workflow.

## Next

- Establish Windows, macOS and Linux build/test coverage and native packaging; validate each on real hardware before claiming support.
- Configurable canvas dimensions and keyboard bindings.
- Typed edit commands and grouped property edits.
- Brush library tags and import compatibility reports.
- Hardware-tested pen sampling, tilt and velocity-aware stabilization.
- Tiled copy-on-write buffers and measured GPU layer compositing; partial stroke texture updates are implemented.
- Release performance baselines on modest reference hardware, including autosave and p95/p99 frame times.
- Tests for disk exhaustion, permission failures and storage migration.
- Selection, clipping and masks, then geometric transforms and liquify.
- Additional brush formats based on versioned fixtures and documented mappings.

Each feature needs observable acceptance criteria. A parsed brush file is not evidence of matching brush behavior; a successful save call is not a substitute for a recovery test.

Implemented after the initial prototype: draggable in-app palettes with persisted layout, pressure curves, persistent brush presets, additional procedural tips and custom window chrome. Remaining UI and editor groups are tracked in the [feature plan](feature-plan.md).

## Mobile apps (later)

- Target Android and iOS/iPadOS after the desktop foundation; device priorities and release dates remain undecided.
- Reuse the Rust drawing core and versioned project format, with an adaptive touch/stylus workspace.
- Separate platform file access, lifecycle, input and rendering before choosing the mobile shell.
- Validate recovery after background suspension and OS termination, memory pressure, sandboxed import/export and tablet pen behavior.

See the [platform plan](platforms.md) for the support matrix and release criteria.

## Community (later)

- Portable material packs with versioned metadata, license, attribution and compatibility.
- Optional brush/material publication, search, tags, previews and downloads.
- Upload limits, validation, reporting and moderation before a public service launches.
- Keep local drawing and existing downloaded materials usable offline without an account.

The [project vision](vision.md) governs scope: responsiveness and recovery come before catalog size or online features.
