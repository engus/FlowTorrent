# Engineering validation

This is a record of observations, not a release approval. Unchecked items remain pending.

## Foundation environment

- Local host: Windows 11 Home x64, build 26300; validation date 2026-10-21.
- Rust: pinned 1.92.0 (`ded5c06cf`, 2025-12-08), edition 2024.
- Visual Studio Build Tools 2022: 17.14.42; MSVC 14.44.35207.
- Windows SDK: 10.0.26100.0.
- UI: Slint 1.18.1, Winit backend, software renderer; accessibility and native system tray enabled. No Qt or WebView.
- Engine: librqbit 9.0.1; Rust TLS, HTTP API client feature disabled.
- Dependency resolution respects the pinned Rust version; exact resolution is committed in Cargo.lock.

The initial compile started before the Windows SDK finished installing and could not link kernel32.lib, then dbghelp.lib. These are environment failures, not failing behavior tests.

## Engine boundary

- Planned controlled swarm: synthetic 512 KiB payload, loopback TCP listener, known SHA-1 piece hashes, isolated temporary directories.
- Swarm tests disable trackers, DHT and local discovery. They must work offline.
- App manifest will be the only session registry. Engine session persistence and fast resume stay disabled; restoration rechecks owned files before resuming.
- New additions use `overwrite=false`; restoration alone can reuse existing owned files with `overwrite=true`.
- Selection uses original file indices. Padding files must never renumber visible files.
- The default engine storage creates empty placeholders for unselected files even when their pieces are not requested. The aligned two-file swarm checks that the unselected file has zero payload bytes. Pieces crossing file boundaries can require bytes from an unselected neighbor; protection of existing files still applies to every path the engine may open. This must be considered by task 3 target validation.

## Private torrents: source observations

In the pinned librqbit source, `Session::make_peer_rx` excludes DHT and LSD streams when `is_private` is true. The live peer implementation ignores received PEX and does not start its PEX sender for private metadata. This is source inspection, **not** a packet-capture test.

The same source explicitly warns that private multi-tracker behavior is incomplete and truncates the tracker list to its first entry. Compatibility with individual private tracker allowlists is not promised.

The controlled private v1 swarm passes. A local fake DHT node observes the public infohash (positive control), then receives no private infohash with DHT enabled. The test sends no DHT queries to public bootstrap nodes.

Wire probes passed for PEX sending, following a PEX-supplied peer, and IPv4 local multicast announcements. Each probe first observes the public behavior as a positive control and then checks that the private torrent does not produce it. The fake PEX peer sends keepalives because the pinned engine disconnects silent peers after ten seconds; its first PEX send is also delayed ten seconds. This avoids mistaking a disconnected observation socket for a privacy result.

These are controlled protocol checks, not compatibility certification for every private tracker. Negative observation windows are bounded: two seconds for DHT/LSD and twelve seconds for PEX, after public positive controls.

## Small rate limits

`cargo run --example limit_probe` reproduces `RateLimitInsufficientCapacity(InsufficientCapacity(1000))` when a 1,000 B/s limiter is asked for a 16,384-byte protocol block. The user chose a 0.02 MB/s minimum (20,000 B/s) for 1.0 rather than a maintained dependency patch. Both limits remain unlimited by default. The adapter validates both requested values before changing either limiter.

## macOS lifecycle investigation

Slint's pinned Winit backend exposes `CustomApplicationHandler`, but that interface has no Finder file-open event. Winit 0.30.13's `WinitApplicationDelegate` implements launch/termination callbacks and asserts that the installed delegate remains its own class. Replacing it with an unrelated application delegate would therefore be unsafe.

The platform adapter must preserve Winit's delegate identity and behavior, and receive `application:openFiles:` / reopen / termination using an AppKit extension or compatible forwarding mechanism. Merely checking argv does not handle an already-running Finder launch.

Pending: implement and verify this adapter on real macOS. No Finder, Dock, menu, Retina or macOS application-launch check has been performed on this Windows host.

## Verification status

| Check | Result |
|---|---|
| Rust/MSVC/SDK installation | Installed |
| Engine behavior tests | Final `cargo test --locked --all-targets`: seven smoke tests and three privacy probes passed, no ignored tests |
| Windows all-targets check | Final `cargo check --locked --all-targets` passed |
| Formatting and lint | `cargo fmt --all -- --check` and `cargo clippy --locked --all-targets -- -D warnings` passed |
| Local Windows GUI launch / live progress | Release window launched; controlled GUI probe displayed Seeding, 524288 / 524288 bytes, progress=1; Alt+F4 ended probe and both sessions |
| Release build | Passed (binary and controlled GUI example); final product binary rebuilt and dumpbin confirms Windows GUI subsystem (2) |
| Release memory measurement | See baseline below |
| Windows/macOS CI matrix | First run compiled/checked both Mac architectures; the IPv4 multicast positive-control probe timed out on Mac ARM. Address-family fixture correction under CI verification; Windows outcome pending |
| macOS GUI and Finder/Dock behavior | Pending real Mac |

## Initial release-process baseline

Windows process counters from PowerShell `Get-Process`, Slint software renderer, small 760×480 technical window. These measure the actual release executables, not debug builds. They do **not** predict the final UI or multi-torrent workload.

| Scenario | PrivateMemorySize64 (bytes) | WorkingSet64 (bytes) |
|---|---:|---:|
| Empty technical window, engine not started | 9,551,872 | 29,290,496 |
| GUI probe, one completed 512 KiB transfer, seeding | 11,599,872 | 34,017,280 |

The second process includes **two** loopback engine sessions (synthetic seeder plus downloader), a two-thread Tokio runtime and the technical UI. The finished client still needs task 9's comparable five-active/twenty-record/hidden-window and long-run measurements.

The Windows UI automation helper exposed the probe's accessibility state correctly; one screenshot captured an overlapping console instead of the Slint surface. Visual/DPI coverage is therefore pending, not inferred from that screenshot. The product release binary now uses the Windows GUI subsystem to avoid a console window; the development probe intentionally remains a console diagnostic.

Local `cargo check` initially hit PermissionDenied overwriting Slint's shared generated `target/debug/build/SLINT_DEFAULT_STYLE.txt`. Removing that generated six-byte file and repeating the check succeeded. No filesystem ACL, OS security or privacy setting was changed.

## Settings and atomic persistence

Settings defaults and exact decimal MB/s parsing pass on Windows. The 0.02 MB/s floor is shared by parsing, storage validation and the engine adapter. OS folders are discovered through `directories`; no alternative download folder is invented on error.

JSON schema version 1 is written through a same-directory temporary file, flushed and synced, then atomically persisted over the destination. The previous valid JSON is separately atomically saved as `.bak`. Damaged JSON and unknown schema versions are preserved and cause an error on both load and save. Unix additionally syncs the parent directory; full machine power-loss durability still depends on the filesystem.

The interrupted-write test starts an owned subprocess, waits until a replacement temporary file exists alongside a valid primary file, terminates the writer, and checks that the primary and any backup are complete JSON with the full known payload. The child entry point is a test fixture only. Local settings tests and the complete test suite pass; macOS settings execution is pending the next CI run.

## Primary references

- [librqbit 9.0.1 session source](https://github.com/ikatson/rqbit/blob/v9.0.1/crates/librqbit/src/session.rs)
- [librqbit private PEX handling](https://github.com/ikatson/rqbit/blob/v9.0.1/crates/librqbit/src/torrent_state/live/mod.rs)
- [Slint 1.18.1 Winit integration](https://github.com/slint-ui/slint/blob/v1.18.1/internal/backends/winit/lib.rs)
- [Winit 0.30.13 macOS application state](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/app_state.rs)
