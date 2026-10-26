> About/support-link follow-up (26 October 2026): [evidence and release next steps](about-and-release-next-steps.md) / [RU](about-and-release-next-steps-ru.md).

> Native white light theme / bottom drawer follow-up (26 October 2026): [evidence](slint-drawer-validation.md) · [Русский](slint-drawer-validation-ru.md). Physical release gates below remain open.

# Engineering validation

This is a record of observations, not a release approval. Unchecked items remain pending.

Latest Slint continuation (26 October 2026): [layout parity, Windows fixes and complete CI evidence](slint-parity-validation.md), with a [Russian report](slint-parity-validation-ru.md). Historical stage evidence follows; physical acceptance gates remain separate.

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
| Windows/macOS CI matrix | Both Mac architectures compiled/checked and passed the complete Stage3 test suite with the separate-socket LSD correction, including public/private outgoing announcements and following an IPv4 peer. Windows also passed; complete matrix run 39584530029 is green |
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

macOS CI isolated the LSD observer failure: a receiver using only SO_REUSEADDR prevents the engine's second socket from binding the same address/port. A minimal socket probe on **both** Mac architectures returned `OSError(48, 'Address already in use')`; enabling SO_REUSEPORT on both sockets made both binds succeed. The observer now sets that option on Unix. The earlier IPv4 alignment alone did not solve the failure. No privacy test was skipped or converted into a successful negative result without its public positive control.

## Settings and atomic persistence

Settings defaults and exact decimal MB/s parsing pass on Windows. The 0.02 MB/s floor is shared by parsing, storage validation and the engine adapter. OS folders are discovered through `directories`; no alternative download folder is invented on error.

JSON schema version 1 is written through a same-directory temporary file, flushed and synced, then atomically persisted over the destination. The previous valid JSON is separately atomically saved as `.bak`. Damaged JSON and unknown schema versions are preserved and cause an error on both load and save. Unix additionally syncs the parent directory; full machine power-loss durability still depends on the filesystem.

The interrupted-write test starts an owned subprocess, waits until a replacement temporary file exists alongside a valid primary file, terminates the writer, and checks that the primary and any backup are complete JSON with the full known payload. The child entry point is a test fixture only. Local settings tests and the complete test suite pass; macOS settings execution is pending the next CI run.

## Primary references

- [librqbit 9.0.1 session source](https://github.com/ikatson/rqbit/blob/v9.0.1/crates/librqbit/src/session.rs)
- [librqbit private PEX handling](https://github.com/ikatson/rqbit/blob/v9.0.1/crates/librqbit/src/torrent_state/live/mod.rs)
- [Slint 1.18.1 Winit integration](https://github.com/slint-ui/slint/blob/v1.18.1/internal/backends/winit/lib.rs)
- [Winit 0.30.13 macOS application state](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/app_state.rs)

## Safe preview and payload storage

Stage 3 uses capability directory handles and a custom librqbit StorageFactory. Preview creates no payload. Confirmation claims every non-padding path with create_new, including unselected placeholders the engine may access. All reads, writes and resizing then use pinned file handles. A conflicting file or a symlink/junction in a parent causes an error; restoration also rejects existing multiply linked files. Padding keeps the original engine index and has no on-disk placeholder.

The controller contract is: claim files after confirmation, persist copied metainfo and manifest ownership, then start the engine and acknowledge success. A crash before manifest persistence may leave empty unacknowledged placeholders; these are conflicts on a fresh add and must never be silently adopted. The manifest remains the sole registry.

Windows behavior checks cover cancellation, concurrent duplicate infohashes under different torrent filenames, empty selection, unsafe paths, normalized name collisions, existing unselected placeholders, a link swapped in after preview, Unicode names, missing targets, pure-v2 rejection and hardlink restoration. Existing transfer, selection, pause/resume and restore checks pass with the custom storage. Final stage checks are recorded in the implementation plan only after verification.

Some newly built test executables initially returned Windows application-control error 4551 before starting. Identical retries ran successfully; no OS security policy or filesystem permissions were changed.
## macOS LSD transport diagnosis

On both macOS CI architectures, actual engine traces show successful socket creation and multicast membership, followed by EHOSTUNREACH (65) sending every IPv4 announcement. A native transmission probe receives the datagram using AF_INET/IP_MULTICAST_IF; the same mapped-v4 destination on AF_INET6 fails with IPV6_MULTICAST_IF (65), and IP_MULTICAST_IF on AF_INET6 is rejected with EINVAL (22).

The narrow override in vendor/librqbit-lsd adds an actual IPv4 socket on macOS and retains IPv6 reception/announcements. Other systems retain the upstream single dualstack socket. Its original Apache-2.0 notice, full license, source commit and patch description are included. Both macOS CI architectures passed the complete Stage3 suite with this correction (run 39584530029); this verifies local network behavior in CI, not Finder/Dock or physical LAN interoperability. Announcement privacy and following an announced IPv4 peer have independent public positive controls and private negative controls; their fixtures use distinct infohashes on the shared multicast port.
## Lifecycle controller

Commands are serialized by one Tokio actor through a bounded channel. The manifest is the only authoritative list; watch snapshots coalesce and are sorted newest first. Current preview and errors also live in the snapshot, so UI reconstruction does not depend on replaying events. Blocking filesystem persistence runs off the actor's async worker.

Real local-engine tests verify that preview creates no payload, confirmation copies metainfo and durably records ownership before transfer, pause persists the requested state, a persistence failure keeps the preview and never acknowledges a new row, and the same confirmation can retry using its claimed file handles. Forget stops transfer and preserves payload. A fault-injecting wrapper around the real engine fails one add with DiskFull; another torrent continues, newest-first order remains stable, and Retry starts the failed job. A completed download continues seeding; pausing it reports Completed.

The aggregate-limit test runs two 2 MiB local jobs through one downloader session at 65,536 B/s. After its initial burst, the combined useful-byte increment stays within one global limit, with a positive transfer control; it does not multiply the limit per job. Pausing both jobs yields zero transfer rates and stable progress. Durable session reconstruction is covered below; the product main window is connected in the UI stage.
## Durable session reconstruction

SessionStore reuses the settings atomic JSON writer and keeps the previous valid manifest as a backup. It validates schema, unique infohashes/positions, fixed metainfo paths and selected indices before accepting a registry. Corrupt/future data is preserved; a missing primary alongside a backup is an error, never an implicit empty session or automatic resurrection of the old copy.

Every restored record first opens its own saved metainfo and registers in RestorePaused. The controller keeps handling commands while the engine rechecks hashes, then resumes only records whose latest durable intent is Active. A missing disk or mismatched metainfo fails that record without clearing the list or redirecting output. Retry reopens that record after the disk returns. Missing owned files are recreated exclusively; foreign files on a new add and multiply linked restore targets remain protected.

The crash test launches an owned real-engine controller subprocess and kills it at six before/after persistence boundaries for Add, Pause and Forget. After restarting, only durable intentions survive; forgotten torrents are not resurrected from leftover metainfo, and an unrelated sentinel is unchanged. The interrupted replacement test from the settings stage separately verifies complete JSON during a killed write. No production fault-injection hooks are added.

Focused Windows restore/ownership/crash tests pass, including restoration without the original torrent, preserved file selection/pause/completion flag, missing-volume retry and missing-owned-file recreation. Final Windows fmt/check/clippy and all-targets tests passed (55 entries: 53 behavior checks and two subprocess fixture entries). The partial-payload test verified 32,768 existing bytes after restart and then downloaded the remainder to the exact original 524,288-byte payload. Desktop UI restart/forced-close coverage follows when stage 6 connects the controller to main.
A regression exposed an upstream initialization race: immediate Pause after New could cancel its first checksum task and leave its visible state indefinitely Checking. The controller now registers new jobs with a private paused-start flag and applies the latest durable intent after the check completes, as it does during restore. Direct engine callers retain normal immediate-start behavior. Immediate pause, partial reuse and completion tests pass with this gate. Hiding/showing also preserves a fatal saved-state error instead of clearing the explanation.
## Shared Slint interface

The product entry point now connects the controller to the main download list, torrent preview/file selection, and four-setting page. RU/EN and system palettes share one layout. A coalesced weak-handle bridge updates changed rows at 2 Hz, preserves selection/scroll, and does not schedule periodic UI work while hidden. Native file/folder dialogs are owned by the main window. Command completion follows durable processing and publication, rather than inferring success from unchanged settings.

Windows controlled GUI checks used the actual UI/controller with an isolated persistent fixture and a loopback seeder. Before confirmation there was no payload; deselecting the only file disabled Download. The download reached Seeding, and Pause reported Completed. Restart and forced termination/restart preserved the paused row without the original torrent. Both graceful closes returned process exit code 0. The 524,288-byte payload matched SHA256 `106eda6faccdbfbe8fb0dfa8f793fdacafda3f3904fafd3f0f7c5932b2a4c090`, including a standard external reader while the client remained open. RU/light and EN/dark were inspected, including native folder selection/cancellation and accessibility labels. The complete Windows scale matrix, keyboard-only addition, and real macOS displays remain manual validation gates.

A Windows interop regression revealed that keeping write handles open blocked ordinary external readers even on pause/completion. Pinned payload handles now relinquish write access there and reacquire it by `ReOpenFile` on the same object when needed; writes never reopen a pathname. A real-engine regression verifies external read sharing and continued transfers. Another regression verifies changing files or destination after a failed manifest save before any transfer; abandoned unacknowledged placeholders remain empty and never overwrite unrelated data.

## Native background lifecycle and one instance

The engine is created only by the process holding the single-instance guard. Subsequent launches send a versioned, length-limited JSON batch over interprocess local sockets. Windows uses a protected owner-only named-pipe DACL; Unix uses a short socket path inside an owner-checked 0700 directory. The listener and process guard have explicit teardown ordering. At most 64 absolute torrent paths and 256 KiB are admitted per batch, with bounded connection deadlines and no reconnect after an uncertain send. Tests cover ordered Unicode/space paths, a delayed listener, malformed/oversize input, guard release and a separate child process. A delivery acknowledgement keeps the bounded input queue from draining into an unbounded native payload backlog.

The Slint SystemTrayIcon owns the native tray/menu-bar icon. Closing the main window hides it; explicit Quit waits for engine ShutdownReady. A separately persisted lifecycle flag records the first background-work explanation, without adding a user setting. Native-event/core-channel waits occur off the UI thread. Cold arguments are queued before subsequent batches. Ctrl+C requests the same graceful shutdown.

Stage 7 CI run 39614530523 passed Windows, Mac ARM and Mac Intel, including both AppKit lifecycle probes. Physical Finder/Dock/Cmd+Q/Retina checks remain manual gates.

On Windows, the isolated real UI showed the explanation once, hid while its PID stayed alive, reopened through a second launch that exited 0, and accepted a new torrent through warm IPC. Enter confirmed its preview; absent sources correctly displayed Waiting for sources, and Pause worked. Closing again hid immediately. The harness invoked the same native Quit event while hidden; the process exited 0 and the original complete payload hash stayed correct. Actual tray-menu clicking, OS file association, the complete scale matrix and keyboard-only system-picker entry remain manual gates. The automation helper reported Search as focused even while the native filename field showed its caret, so that picker test is not claimed passed.

On macOS the adapter extends the existing Winit delegate with a no-new-ivars subclass: it preserves Winit's identity/launch/termination methods, handles file-open and Dock-reopen callbacks, and defers application termination until the engine stops. A dedicated CI executable checks delegate identity, ordered synthetic AppKit file messages, reopen delivery and the hidden event loop. This is additional automated coverage, not a substitute for real Finder/Dock/Retina and signed-app testing.

## Completion notices and bounded diagnostics

Completion uses selected bytes and a genuine Seeding/Completed state. The controller marks completion in the authoritative manifest before sending its ID/name to the notice dispatcher. Disabled notices also mark completion, preventing old notices after re-enabling. The crash contract is at most once: persistence followed by a crash before delivery can omit a notice. A failed flag save emits no notice, leaves seeding active, suppresses identical repeated UI errors and retries persistence; recovery clears that save's error.

The dispatcher has one worker and a bounded 32-entry queue. A notification-service or permission failure is a local warning and cannot set torrent Error. Windows uses native Toast XML text nodes and the application's own AppUserModelID; the installer must assign the same ID to its Start Menu shortcut ([Microsoft requirements](https://learn.microsoft.com/en-us/windows/win32/shell/enable-desktop-toast-with-appusermodelid)). macOS uses UserNotifications with asynchronous authorization and a bundle-ID guard; unbundled visual tools use a no-op sink. No notice launches payload files. Installed-system delivery and authorization-denial checks remain packaging/manual gates.

The local warning log rotates at 1 MiB and retains one previous file; oversized events are truncated at approximately 8 KiB. IO causes remain typed through engine context for disk-full, permission and missing-volume messages. Missing original metadata has an invalid-torrent explanation. Only recoverable row errors offer Retry. Startup failures use localized text, preserve technical detail in the local log and return a failing process status.

Windows final formatting, clippy with warnings denied, all-targets check and all-targets tests pass: 74 entries (71 behavior checks and three subprocess fixtures). Real-seeder cases verify durable completion before event delivery, no duplicate after restart/pause, and no notification or transfer failure while completion-state saving fails. Log bounds and recoverable-error policy have direct regressions. [Stage 8 CI run 39619141807](https://github.com/engus/FlowTorrent/actions/runs/39619141807) passed Windows, Mac ARM and Mac Intel, including native Mac notification compilation and both AppKit probes.

## Partial reliability milestone

Four additional regressions first reproduced failures, then passed locally. Rejected commands preserve the original corrupt-state explanation. A nonrecursive metadata preflight rejects excessive bencode nesting before the upstream recursive decoder and treats string payloads as opaque bytes. On Windows a complete owned file can be restored while an ordinary external reader remains open; capability-bound handles and ownership checks remain in place. Retry after an actual write-sharing error uses paused-first restoration, so an immediately following Pause cannot strand the checksum task in Checking.

The four tests in `tests/reliability.rs`, formatting and all-targets clippy passed. The initial full local test run was interrupted after prolonged parallel linking; later complete CI runs below supersede that pending automated gate. Stage 9 still has manual and overnight gates. See [the development handoff](development-handoff.md).

### Stage 9 continuation: real swarm failures

[CI run 39649972678](https://github.com/engus/FlowTorrent/actions/runs/39649972678) subsequently passed the full checks for `8c94a06` on Windows x64, Mac ARM and Mac Intel.

New controlled-swarm cases exercise a TCP connection cut and automatic reconnection, a deliberately corrupted piece on restore, and file selection across a piece boundary. The corruption case retains exactly 507,904 of 524,288 verified bytes before downloading the single damaged 16,384-byte piece. The relay affects only fixture TCP connections; it does not change the host's networking or model OS sleep.

The boundary case reproduced a real adapter bug: a 50,000-byte selected file appeared as 53,616 bytes because upstream totals include shared-piece bytes from its unselected neighbour. Snapshots now sum verified per-file progress for selected non-padding files. The test checks exact selected totals both at completion and after paused restoration without a source.

A test-only wrapper around the real capability-backed storage raises StorageFull and PermissionDenied on actual payload writes. For both failures the real engine reports the actionable code, a second torrent completes correctly, no failed payload write is acknowledged, and restoring/retrying completes the original bytes. No production fault switch is introduced and no host disk is filled.

### Stage 9 automated milestone — 22 October 2026

Source revision [`0c990ba`](https://github.com/engus/FlowTorrent/commit/0c990baa34b7f1982affed38a88f491da35bfcc6) passed [Check 39683564507](https://github.com/engus/FlowTorrent/actions/runs/39683564507) on Windows x64, Mac ARM and Mac Intel: formatting, all-targets check, Clippy with warnings denied, and all-targets tests. Each job reports 86 passing test entries, zero ignored; these include three subprocess fixture entry points. Both Mac AppKit probes also passed. [Release validation 39683655107](https://github.com/engus/FlowTorrent/actions/runs/39683655107) built the application and probe on all three targets and passed all five resource scenarios plus an explicit 1.25-scale readiness check. These are build/test artifacts, not published installers.

A real Mac regression reproduced `EMFILE` (errno 24): a child limited to 64 soft file descriptors could not add a 256-file torrent. The intentional failing [test-only run](https://github.com/engus/FlowTorrent/actions/runs/39679597494) is retained on `codex/stage9-nofile-limit`. Unix engine initialization now calls the upstream `try_increase_nofile_limit()` once, respecting the inherited hard limit and Darwin kernel cap; inability to raise it logs a warning. The test changes only its own child limit, never global OS settings. Both Macs pass the regression and the full suite; [the first fully green fix run](https://github.com/engus/FlowTorrent/actions/runs/39681124528) preceded the same tree being brought to main.

The aggregate upload test uses two independent real downloaders and one shared 65,536 B/s uploader limit. Its positive-transfer counter now sums verified per-file bytes: the upstream raw initialization counter can reset after scanning and was an invalid fixture control. Production transfer counters were not changed to hide that reset.

Release measurements, raw CSV and environment records are in [performance.md](performance.md). Hidden-window measurements occur after the first painted frame; hiding before painting understated retained UI memory. Scale metadata is captured after the UI event loop initializes, rather than reporting an early default of 1. A real release probe and every release CI target verify an override of 1.25.

Native Windows list layouts with twenty paused records were inspected at Slint scale overrides 1, 1.25, 1.5 and 2. Main actions and row controls remained visible, and the list scrolled. Settings were inspected at 1, 1.5 and 2, including expanded speed controls at 2; Tab gave a visible focus ring and Enter opened Settings. These child-process overrides leave OS settings unchanged. They do not complete per-monitor DPI/native-picker checks, the full keyboard-only addition workflow, or physical Mac display coverage.

The first continuation full local suite passed 82 entries before later probe/Unix regressions were added. Subsequent local all-targets launches were blocked by Windows App Control error 4551 before tests executed, including an unchanged retry. They are not counted as passes. No security policy or ACL was changed. Local strict Clippy/check and release application/probe builds passed; the final full-suite evidence is the green three-platform CI above.

Overnight download/seeding, real sleep/wake, installed notifications/associations and physical Mac Finder/Dock/Retina checks remain open in the [release checklist](release-checklist.md). Short probes and relay reconnection do not substitute for them.

## Installed Mac follow-up — 23 October 2026

Subsequent bounded physical evidence is recorded in the [Mac acceptance](mac-validation-2026-10-23.md), [notification checks](mac-notifications-2026-10-23.md) and [Dock/keyboard/Retina checks](mac-ui-acceptance-2026-10-23.md). These update the historical pending statuses above for this Apple Silicon Mac/macOS 27 account only. Dock return/Quit have user confirmation and process-exit evidence; current dark Retina layouts and basic keyboard actions were inspected. Cmd+O fails with the Russian input source and remains open. Other supported systems, ordinary displays, the full keyboard/theme/locale matrix and overnight acceptance are not inferred from these checks.
