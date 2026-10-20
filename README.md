# FlowTorrent

A lightweight, ad-free BitTorrent client for Windows and macOS, built with Rust, librqbit, and Slint. Simple downloads, a clean interface, and quiet background operation.

## Project status

FlowTorrent is in the planning stage. Application code and installable releases are not available yet.

The first version focuses on opening a `.torrent` file, choosing where to save it, downloading, and continuing to seed in the background. Settings are limited to the download folder, completion notifications, and optional global download/upload speed limits.

## Development documents

- [FlowTorrent 1.0 specification](docs/spec-1.0.md) — agreed behavior, scope, supported platforms, and release criteria, in Russian.
- [FlowTorrent 1.0 implementation plan](docs/superpowers/plans/2026-10-21-flowtorrent-1.0.md) — ordered development tasks, interfaces, checks, and milestones, in Russian.

## License

FlowTorrent's own code and documentation are licensed under the [MIT License](LICENSE). Dependencies retain their own licenses.

The planned UI dependency, Slint, will be used under its [Royalty-free license](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md). Its attribution badge must remain visible on the public project/download pages used for distribution. librqbit is licensed under Apache-2.0. Distribution notices will be assembled from the exact dependencies in `Cargo.lock` before release.

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev/)
