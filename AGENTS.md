# Parfait project direction

Read [docs/vision.md](docs/vision.md) before making product or architecture decisions.

- Build a lightweight, low-latency, pen-first drawing tool. SAI-class lightness is an aspiration, not a measured performance claim.
- Prioritize stroke responsiveness, bounded memory, reliable local autosave and crash recovery over feature count.
- Keep the drawing path local and usable offline, without an account or network dependency.
- External AI processing is forbidden. Never upload artwork to AI services, add cloud-AI clients, or automatically download models. Improve the local tools; any model processing must use explicitly selected local resources.
- Windows, macOS and Linux are first-class targets; preserve a reusable Rust core for future Android and iOS/iPadOS apps. Distinguish intended support from tested support.
- Plan an optional community for sharing brushes and materials. Keep community/network services outside the rendering and storage core.
- Public implementation and documentation must use independently authored code and redistributable assets. Keep third-party reverse-engineering material out of the repository and do not attribute independently written features to proprietary implementation details.
- Measure rendering changes with reproducible release workloads, check pixel equivalence, and preserve recovery tests. Never present a microbenchmark as end-to-end pen latency or competitive parity.
