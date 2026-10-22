# FlowTorrent

A lightweight, ad-free BitTorrent client for Windows and macOS, built with Rust, librqbit, and Slint. Simple downloads, a clean interface, and quiet background operation.

## Project status

FlowTorrent is under development. The Slint interface can preview local torrent files, select files and a destination, download, seed, pause, and restore the saved session. Single-instance file delivery and background window handling are implemented. Native completion notifications are wired; installed-system validation and installable releases are still in progress.

Use **Add torrent** to choose a `.torrent` file, or the adjacent magnet icon to
paste a magnet link. Magnet metadata is fetched before the file-selection and
destination preview; lookup can be cancelled, and downloading starts only after
confirmation. BitTorrent v1 and hybrid magnets with a v1 hash are supported.

The first version focuses on adding a torrent, choosing where to save it, downloading, and continuing to seed in the background. Settings are limited to the download folder, completion notifications, and optional global download/upload speed limits.

Stage 9's automated reliability checks and bounded release measurements pass on Windows x64, Mac ARM and Mac Intel. Overnight operation, real sleep/wake and installed-system checks remain open; see the reports below.

## Build the technical preview

For a locally runnable Mac app with a Dock icon, menu bar controls and `.torrent`
file registration, run `./scripts/package-macos.sh`, then open the printed
`FlowTorrent.app` path. See [local Mac development](docs/local-macos.md) for
requirements, rebuilding, Intel builds and verification. This creates an ad-hoc
signed local test app; public distribution remains pending.

Install Rust through rustup; the repository selects Rust 1.92.0. Windows also needs the Visual Studio 2022 C++ Build Tools and Windows SDK. macOS needs the Xcode Command Line Tools. The macOS deployment target is 14.0.

```sh
cargo run --locked --release
cargo test --locked --all-targets
```

The first command opens the application interface. For a synthetic local transfer without public trackers, use `cargo run --locked --example ui_smoke -- <absolute-fixture-directory> --open`. This harness uses its own persistent registry and a loopback seeder; omit `--open` to test restoration without recreating the original torrent. `--en` and `--dark` select visual test variants; the application itself follows the OS language and theme. The smaller `foundation_smoke` example remains available for engine diagnostics.

See [engineering validation](docs/engineering-validation.md) for actual results, engine constraints, and pending macOS checks. This preview is not a public 1.0 release.

## Development documents

- [FlowTorrent 1.0 specification](docs/spec-1.0.md) — agreed behavior, scope, supported platforms, and release criteria, in Russian.
- [FlowTorrent 1.0 implementation plan](docs/superpowers/plans/2026-10-21-flowtorrent-1.0.md) — ordered development tasks, interfaces, checks, and milestones, in Russian.
- [Development handoff](docs/development-handoff.md) — saved progress, remaining validation, and where to resume, in Russian.
- [Resource measurements](docs/performance.md) — reproducible release workloads, measured results and raw CSV.
- [Release checklist](docs/release-checklist.md) — verified evidence and outstanding release gates.

## License

FlowTorrent's own code and documentation are licensed under the [MIT License](LICENSE). Dependencies retain their own licenses.

Slint is used under its [Royalty-free license](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md). Its attribution badge must remain visible on the public project/download pages used for distribution. librqbit is licensed under Apache-2.0. Distribution notices will be assembled from the exact dependencies in `Cargo.lock` before release.

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev/)
