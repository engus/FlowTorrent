[Русский](README-ru.md)

# FlowTorrent

A free, ad-free BitTorrent desktop client for Windows and macOS. Add a `.torrent` file or magnet link, choose the files and destination, and keep downloading or seeding in the background.

FlowTorrent uses one shared [Slint](https://slint.dev/) interface on both platforms, with Rust and librqbit underneath.

## What it does

- BitTorrent v1 torrents and magnets, including hybrid metadata with a v1 hash; file and destination preview before downloading.
- Pause, resume, retry and session restoration, with data checked before active transfers resume.
- Local name search, sorting and All / Downloading / Completed views; Start all and Stop all affect the whole list.
- A collapsible, resizable sidebar and bottom details drawer with per-file progress and search.
- Background transfers, optional completion notifications, and tray/menu bar download and upload rates with the estimated time until all active downloads finish.
- Remove from list while keeping files, or explicitly move owned files to the system Trash / Recycle Bin after confirmation.
- Autosaved settings: System / Light / Dark appearance, System or one of 14 interface languages, launch at login, new-download folder, notifications and global speed presets.

Upload percentage measures the current engine session; it is not a lifetime sharing ratio. There is no automatic stop at a ratio. See the [user guide](docs/user-guide.md) for controls, safe removal and recovery.

## Status and installation

The current package version is **0.1.0, a development test build**. There is no public 1.0 release. Source support targets Windows 10 22H2 / Windows 11 x64 and macOS 14 or later on Apple Silicon and Intel. This target list is not a claim that every physical acceptance check has passed.

Windows test packages use a **per-user Inno Setup installer**. On Mac, open the architecture-matching DMG and drag **FlowTorrent.app** to **Applications**; the DMG is a distribution container. Local Mac builds can also create the `.app` directly. Windows test builds are unsigned; Mac test builds use ad-hoc signing, without Developer ID or notarization. Public signing credentials have not been provided.

See [test packaging](docs/test-packages.md) and [local Mac development](docs/architecture/local-macos.md) for build and installation details. Physical Windows, real DPI/display, supported-Mac coverage, overnight operation and other release gates remain recorded in the [release checklist](docs/validation/release-checklist.md). Historical CI and native observations are linked from the [documentation index](docs/README.md); they do not certify a newer revision.

## Build from source

The root Cargo workspace builds [desktop](apps/desktop/README.md) by default. [Website](apps/website/README.md), [mobile](apps/mobile/README.md) and [shared crates](crates/README.md) have reserved locations. See [the folder layout](docs/architecture/README.md). Local design work stays in ignored `_work/` and must be preserved.

Install Rust through rustup; `rust-toolchain.toml` pins **Rust 1.92.0**. Windows needs Visual Studio 2022 C++ Build Tools and the Windows SDK. macOS needs the Xcode Command Line Tools. Keep Cargo parallelism at two jobs and use the locked dependencies.

Windows / PowerShell:

```powershell
cargo run --locked --release -j 2
```

macOS:

```sh
export MACOSX_DEPLOYMENT_TARGET=14.0
cargo run --locked --release -j 2
# For a local app bundle with platform integration:
./scripts/package-macos.sh
```

Repository checks (set the deployment target above on macOS):

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -j 2 -- -D warnings
cargo test --locked --all-targets -j 2
cargo build --locked --release -j 2
```

On macOS, also run `cargo run --locked --example macos_lifecycle_probe -j 2`. Tests use controlled local swarms. Packaging and notices require additional tools described in the [packaging guide](docs/test-packages.md).

## Documentation and author

Start with the [documentation index](docs/README.md), [user guide](docs/user-guide.md), [localization guide](docs/architecture/localization.md), and original [English / Russian brandbooks](docs/brand/README.md).

By **Yevgeniy Golota · engus** — [author website](https://golota.me). The author website is not a project download site. [Report an issue](https://github.com/engus/FlowTorrent/issues) on GitHub. **Support the author** is currently a disabled placeholder in About; the Ko-fi destination is pending. Every feature is free.

## License and credits

FlowTorrent's own code and documentation use the [MIT License](LICENSE). Dependencies keep their licenses; see [third-party notices](THIRD_PARTY_NOTICES.md) and the [dependency policy](docs/architecture/dependency-policy.md). librqbit uses Apache-2.0; Slint uses Royalty-free 2.0; [Lucide controls](assets/icons/lucide/README.md) use ISC / MIT; [Nunito](assets/fonts/OFL.txt) uses SIL OFL-1.1. Keep the official Slint attribution in the app and public project/download pages.

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev/)
