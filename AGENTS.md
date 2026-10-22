# AGENTS.md

Guidance for coding agents working on FlowTorrent: a minimal, ad-free BitTorrent desktop client for Windows and macOS, built with Rust, librqbit and Slint.

## Read first

- [docs/spec-1.0.md](docs/spec-1.0.md) defines the agreed behavior and scope. Where the plan or the code disagrees with it, the spec wins.
- [docs/development-handoff.md](docs/development-handoff.md) records the current state, the verified baseline and where to resume.
- [docs/superpowers/plans/2026-10-21-flowtorrent-1.0.md](docs/superpowers/plans/2026-10-21-flowtorrent-1.0.md) is the ordered task plan. Its checkboxes track progress; tick one only after the work is verified.
- [docs/release-checklist.md](docs/release-checklist.md) and [docs/engineering-validation.md](docs/engineering-validation.md) record actual evidence. Don't record a CI build as proof of a physical check such as Finder, the Dock, the tray, Retina or DPI.

## Build and verify

The toolchain is pinned to Rust 1.92.0 in `rust-toolchain.toml`. Always pass `--locked`.

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -j 2 -- -D warnings
cargo test --locked --all-targets -j 2
cargo build --locked --release -j 2
```

- Keep local parallelism at `-j 2`. Linking with more jobs has overloaded the dev machine before.
- macOS builds need `MACOSX_DEPLOYMENT_TARGET=14.0`.
- On macOS, also run `cargo run --locked --example macos_lifecycle_probe`.
- CI (`.github/workflows/ci.yml`) runs fmt, check, Clippy and the tests on Windows x64, Mac ARM and Mac Intel. Every change must keep all three green.
- The resource probes run in `release-validation.yml`, which is triggered manually.
- Tests use only local, controlled swarms. Never make public trackers, DHT or internet access a condition of CI.
- If the Slint build script hits PermissionDenied on `target/*/build/SLINT_DEFAULT_STYLE.txt`, wait until Cargo has fully exited. Then delete only that six-byte generated file (contents `fluent`) and rerun. Never change ACLs or system protection to get around a block.

## Layout

| Path | Purpose |
| --- | --- |
| `src/domain.rs`, `src/error.rs` | Shared types, states, `AppError` → localizable `ErrorCode` |
| `src/app/` | `AppController` (the single owner of the registry and command loop), add preparation (`targets.rs`), session store, restore |
| `src/engine/` | The `TorrentEngine` trait and the librqbit adapter. All librqbit calls stay here |
| `src/engine/storage.rs` | Capability-based storage that writes only through pre-claimed file handles |
| `src/platform/` | Single instance and IPC, tray, notifications, AppKit delegate hooks, Windows app ID |
| `src/ui/`, `ui/*.slint` | Slint UI, display models, RU/EN strings |
| `src/settings.rs`, `src/persistence.rs` | The four settings and atomic JSON writes |
| `tests/`, `tests/support/` | Integration tests with a local seeder and fixtures |
| `examples/` | Smoke harnesses and the `measure_resources` release probe |
| `vendor/librqbit-lsd/` | Patched LSD crate for macOS IPv4 multicast; see its `PATCHES.md` |

## Invariants: do not break

- **Path safety.** All payload I/O goes through `PreparedAdd` and the pinned `cap-std` folder handles, never through a joined path string. Keep rejecting `..`, absolute paths and drive/UNC paths, Windows device names, names that clash after case/NFC normalization, symlinks and reparse points. On restore, also keep rejecting hard links.
- **No payload deletion.** `SecureStorage::remove_file` and `remove_directory_if_empty` must keep failing. "Remove from list" keeps the downloaded files.
- **No overwriting.** New adds use `create_new`. Only `AddMode::RestorePaused` for a manifest record the app owns may open existing files.
- **One source of truth.** The app manifest (`session.json`) owns the torrent list and desired states. librqbit persistence and fastresume stay disabled.
- **Durable intent first.** Persist the manifest change before reporting success. Writes use `atomic_write_json`: temp file, sync, atomic replace, `.bak`.
- **Damaged state is never overwritten.** Damaged or future-version files cause an error and are left as they are.
- **Notifications are at most once.** Persist `completion_notified` before notifying. A notification failure never affects transfers.
- **Restore is paused first.** Restored and retried torrents start paused and resume only after Checking finishes, if the desired state is Active. Don't call upstream unpause while a torrent is in Error.
- **Threading.** Slint runs on the main thread and the engine in Tokio. Commands go through a bounded mpsc and snapshots through `watch`. Don't send Slint handles across threads, and never block the UI thread on the network or the disk.
- **IPC stays private.** On Windows the pipe has an owner-only DACL. On Unix the socket sits in a `0700` folder owned by the user, and messages are bounded. Paths are passed as structured data, never through a shell.
- **Keep the LSD fix** in `vendor/librqbit-lsd` until upstream passes both Mac architectures.
- **Pinned versions.** librqbit 9.0.1, Slint 1.18.1 and the other `=`-pinned crates are upgraded only in a separate, re-verified change.

## Scope

1.0 is deliberately small. There are exactly four settings: download folder, completion notifications, and global download and upload limits. The minimum enabled limit is 0.02 MB/s, and 1 MB/s means 1,000,000 bytes/s. Don't add any feature from spec §12 without an explicit decision. That list includes magnet links, drag and drop, RSS, search, launch at login, auto-update, categories, queues and network settings.

## Code style

- Match the surrounding code: terse modules, few comments, and comments that explain *why*. Every `unsafe` block needs a `// SAFETY:` comment.
- Map errors to `AppError` and map them to user-facing text in `src/ui/i18n.rs`. Every user-facing string needs both a Russian and an English variant via `Locale::text(ru, en)`. Paths and file names are never translated.
- Work test-first: write a failing test, then make it pass, keeping each fix in a small commit with a reproducing test.
- Commit messages use Conventional Commits in the imperative (`feat:`, `fix:`, `test:`, `docs:`, `build:`).

## Documentation

- English is the primary language. A Russian version lives next to its English original with a `-ru.md` suffix, for example `docs/spec-1.0.md` and `docs/spec-1.0-ru.md`.
- When you change a document that has a `-ru.md` pair, update both, and keep each pair linked to the other at the top.
- `claude-audit.md` in the root records the latest code audit and its open findings.

## Releases and outward actions

- Never publish a GitHub Release, push tags or upload installers automatically. Publishing is a separate action that the maintainer approves.
- Signing credentials (Windows certificate, Apple Developer ID and notarization) are not provided. Don't commit or log secrets. Unsigned builds are test builds and must be labeled as such.
- Keep the author's MIT `LICENSE`, the dependency licenses and the visible "Made with Slint" badge required by the Slint Royalty-free license.
