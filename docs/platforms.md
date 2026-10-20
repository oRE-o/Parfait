# Platform plan

Parfait is intended to support Windows, macOS and Linux as first-class desktop platforms. Mobile apps are a later phase, with Android and iOS/iPadOS as intended targets. This is a product direction, not a claim that all platforms work today. Release dates and minimum OS versions are not yet set.

## Current status

| Target | Current evidence | Work before a supported release |
| --- | --- | --- |
| Windows | Local build, core/storage tests, process-kill recovery tests and rendered workspace inspection | Installer/distribution checks, real stylus testing and broader hardware coverage |
| macOS | Apple M1/arm64 debug and release builds, native UI/drawing/restart smoke checks, workspace tests and process-kill recovery tests | Real stylus/latency, broader graphics/window/DPI and multi-monitor checks, distribution signing/notarization and more hardware |
| Linux | Intended desktop target; no native build or hardware validation yet | Build dependencies, X11/Wayland checks, distribution packaging, pen input and recovery validation |
| Android | Planned; no mobile frontend or build yet | Mobile shell, lifecycle/storage integration, adaptive UI and device validation |
| iOS/iPadOS | Planned; no mobile frontend or build yet | Mobile shell, lifecycle/storage integration, adaptive UI and device validation |

CPU architecture coverage will be declared per tested artifact. A successful build for one architecture does not establish support for another.

## Shared foundation

- Keep `parfait-core` independent of OS, windowing and GUI APIs. Reuse brush behavior, document state and editor commands.
- Use one versioned project format with migration tests and cross-platform fixtures. Verify projects saved on one platform can be opened and edited on another.
- Keep storage encoding and validation reusable. Put platform-specific file access, locking and save scheduling behind explicit boundaries where required.
- Treat the desktop eframe frontend as the current implementation. Evaluate mobile UI and graphics backends before committing to a shared frontend; do not couple the drawing engine to that choice.

## Platform adaptation

- **Files:** use platform-appropriate application-data directories, native pickers and sandbox-aware import/export. Do not assume every selected document is a permanent writable filesystem path. macOS data-directory conventions still need implementation; the prototype currently uses its generic desktop fallback.
- **Input:** normalize mouse, pen and touch samples, including pressure and available stylus metadata. Separate drawing from pan/zoom/rotate gestures; validate palm rejection and device behavior on hardware.
- **Commands and layout:** preserve familiar desktop actions while following platform modifier-key conventions. Provide accessible touch targets, responsive panels and gesture controls for tablets and phones.
- **Lifecycle and recovery:** save incrementally during editing. Request saves on background transitions when possible, but never depend on a normal exit callback. Test termination, suspension/resume and storage failures separately.
- **Performance:** measure memory use, latency and battery impact on representative devices. Tiled buffers and partial updates should precede large-canvas promises on mobile.

## Delivery sequence and gates

1. Add native Windows, macOS and Linux CI jobs for formatting, linting, tests and builds. The repository currently contains a CI template; hosted CI is not enabled.
2. Validate drawing, layer operations, shortcuts, navigation, import/export and recovery on each desktop OS. Exercise both X11 and Wayland on Linux where supported.
3. Produce platform-specific packages and document minimum OS versions, architectures, dependencies and signing/distribution status.
4. Prototype mobile shells using the shared core, then test touch/stylus workflows, sandboxed files, memory pressure and OS termination on actual devices.

Declare a platform supported only after its build, package, interaction and recovery checks pass. Document known gaps per release. Desktop process-kill tests alone do not establish mobile lifecycle safety or power-loss durability.

## Edit-menu integrations (2026-10-20)

This change adds local clipboard (`arboard`), screen capture (`xcap` 0.9.8), and optional gamepad input (`gilrs` 0.11.2). The Windows build uses gilrs XInput. Native content rasterization uses Rust `ttf-parser` and `tiny-skia`; it does not use OS font discovery or a network service. Fonts selected for a document are embedded; a custom UI font instead remains a local path preference.

For Debian/Ubuntu, xcap lists `pkg-config libclang-dev libxcb1-dev libxrandr-dev libdbus-1-dev libpipewire-0.3-dev libwayland-dev libegl-dev`; gilrs also needs `libudev-dev`. These are additional to the existing desktop windowing requirements. Other distributions should use equivalent development packages. xcap's 0.9.8 implementation table marks Wayland screen capture as available with incomplete support in some scenarios. Parfait has not verified the Wayland path, screen-permission prompts, or real controllers on macOS/Linux. Display-capture errors appear in the picker without changing artwork.

Wireless discovery delegates to the operating system's Bluetooth settings. Device registration refers to connected gamepads and their button mappings; it does not imply that Parfait implements a Bluetooth pairing stack, tablet-driver installation, or pressure input from a gamepad.
