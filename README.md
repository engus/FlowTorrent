# FlowTorrent

A lightweight, ad-free BitTorrent client for Windows and macOS, built with Rust, librqbit, and Slint. Simple downloads, a clean interface, and quiet background operation.

## Project status

FlowTorrent is under development. The technical foundation has a Slint window, a librqbit adapter, and controlled download/protocol tests. The complete download interface and installable releases are not available yet.

The first version focuses on opening a `.torrent` file, choosing where to save it, downloading, and continuing to seed in the background. Settings are limited to the download folder, completion notifications, and optional global download/upload speed limits.

## Build the technical preview

Install Rust through rustup; the repository selects Rust 1.92.0. Windows also needs the Visual Studio 2022 C++ Build Tools and Windows SDK. macOS needs the Xcode Command Line Tools. The macOS deployment target is 14.0.

```sh
cargo run --locked --release
cargo test --locked --all-targets
```

The first command currently opens the technical window. For a synthetic local transfer with UI progress, use `cargo run --locked --release --example foundation_smoke`; close its window after the transfer finishes. It runs a temporary seeder and downloader and verifies the resulting bytes without public trackers.

See [engineering validation](docs/engineering-validation.md) for actual results, engine constraints, and pending macOS checks. This preview is not a public 1.0 release.

## Development documents

- [FlowTorrent 1.0 specification](docs/spec-1.0.md) — agreed behavior, scope, supported platforms, and release criteria, in Russian.
- [FlowTorrent 1.0 implementation plan](docs/superpowers/plans/2026-10-21-flowtorrent-1.0.md) — ordered development tasks, interfaces, checks, and milestones, in Russian.

## License

FlowTorrent's own code and documentation are licensed under the [MIT License](LICENSE). Dependencies retain their own licenses.

Slint is used under its [Royalty-free license](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md). Its attribution badge must remain visible on the public project/download pages used for distribution. librqbit is licensed under Apache-2.0. Distribution notices will be assembled from the exact dependencies in `Cargo.lock` before release.

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev/)
