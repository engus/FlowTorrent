[Русская версия](development-handoff-ru.md)

> Latest local work (27 October): [workspace and tray time](validation/monorepo-eta-validation.md), [checking percentages](validation/checking-progress-validation.md), [double-click details](validation/slint-double-click-validation.md), [lifecycle probe timing](validation/native-lifecycle-probe-2026-10-27.md). The maintainer confirmed the latest Mac build working and authorized commits, push and full three-platform CI with installer uploads disabled. No main merge or release is authorized by that request.

> Native menus and owned add window (27 October): [validation](validation/native-polish-validation.md) / [Russian](validation/native-polish-validation-ru.md). Branch `codex/slint-native-polish-20261027`, draft PR #6; ordinary-app tray restart and native popup dispatch verified. No release.

> Menu-bar/tray preference (26 October): [validation](validation/tray-preference-validation.md) · [Русский](validation/tray-preference-validation-ru.md). Physical Mac hidden-download check passed; remaining platform release gates stay open.

> Latest native layout (26 October 2026): [bottom drawer validation](validation/slint-drawer-validation.md) · [Русский](validation/slint-drawer-validation-ru.md).

> Latest Slint UI work (26 October 2026): [handoff](slint-redesign-handoff.md) · [layout parity and full CI](validation/slint-parity-validation.md) · [Русский](validation/slint-parity-validation-ru.md). Historical baseline follows.

# Continuing FlowTorrent development

State as of 22 October 2026: stages 1–8 are implemented; the automated part of stage 9 has been verified and pushed to `main`. Release 1.0 is not ready yet: long-running and physical checks remain, followed by stage 10 packaging.

## Verified baseline

Sources `0c990baa34b7f1982affed38a88f491da35bfcc6` passed [Check 39683564507](https://github.com/engus/FlowTorrent/actions/runs/39683564507) on Windows x64, Mac ARM and Mac Intel: fmt, all-targets check, Clippy with `-D warnings`, all-targets tests. Each job has 86 passing test entries, 0 ignored; three entries are child-fixture entry points. Both Mac AppKit probes passed as well.

[Release validation 39683655107](https://github.com/engus/FlowTorrent/actions/runs/39683655107) built the application and the measurement tool on all three architectures, checked five scenarios and a separate 1.25 scale. The final documentation commit adds reports and CSV without changing these sources. There are no installers yet, and no public GitHub Release has been created.

## What was done in stage 9

- Preserving the original corrupted-session error when a command is refused, protection against excessive bencode nesting, recovery while an external Windows reader holds a file open, an immediate pause after retry without getting stuck in Checking.
- Real local downloads with a TCP connection dropped and restored, recheck of one corrupted piece, controlled StorageFull/PermissionDenied on real writes and independent operation of a second torrent. The system disk was not filled and the OS network was not changed.
- Fixed selected-file progress at a piece boundary: exactly 50,000 bytes instead of 53,616 including neighbouring bytes; verified after restoring without a source.
- Verified the global upload limit across two independent downloaders. Fixed the test fixture's counter: it counts verified file bytes, not the upstream scan counter that gets reset.
- Reproduced a real `EMFILE` on a Mac for 256 files with a soft FD limit of 64. Unix initialization now calls `librqbit::try_increase_nofile_limit()` once, within the inherited hard limit and the Darwin limit. Both Macs pass the regression; global OS settings are not changed. The `codex/stage9-nofile-limit` branch keeps the red→green history.
- Added the release-only `measure_resources` tool, a separate seeder process, CSV/metadata and the manual Release validation workflow. Fixed measuring the hidden window after the first render and reading the scale after the event loop starts.
- Saved [real measurements](validation/performance.md) on three CI targets and local Windows, plus 300 seconds of hide/show. Locally, an empty window takes about 34 MiB working set, five active downloads 39 MiB. This is a controlled workload without public discovery and IPC, not a universal memory bound for the client.
- On Windows, reviewed a 20-entry list with Slint overrides 1/1.25/1.5/2; settings at 1/1.5/2, expanded limits at 2, Tab/Enter and visible focus. The OS's own DPI settings were not changed.

Detailed evidence and verification limits: [engineering-validation.md](validation/engineering-validation.md), [release-checklist.md](validation/release-checklist.md), [plan](superpowers/plans/2026-10-21-flowtorrent-1.0.md).

## Where to resume

1. Check the latest CI, read the specification and the open stage 9 items. Do not repeat automated checks that are already done unless there are new changes or failures.
2. An overnight download/seeding run with data verification; real sleep/wake on Windows and Mac. The five-minute hide/show and the TCP drop do not replace these checks.
3. The full keyboard path through the system picker, the real tray menu and Windows per-monitor DPI. On physical Macs: Finder/Dock/Cmd+Q, an ordinary display and Retina. The CI AppKit probe does not replace these actions.
4. Stage 10: Inno Setup for Windows, `.app`/DMG for Mac ARM/Intel, `.torrent` associations, the `com.engus.flowtorrent` identifier, notifications, exact licenses from Cargo.lock, versions and SHA-256. Test artifacts only at first; automatic publication of a GitHub Release is not allowed.
5. After packaging: clean install, update preserving the session, uninstall without deleting data; notification delivery/denial. Windows certificates and Apple Developer ID/notarization have not been provided. Finish the release checklist and a fresh review of the whole branch before announcing 1.0.

Optimize copying of large previews only after measuring. On a future Rust upgrade, separately check the stability of the IPC namespace that uses `DefaultHasher`; the current toolchain version is pinned.

## Recorded decisions and local environment

Rust 1.92, librqbit 9.0.1, Slint 1.18.1. Four settings: folder, notifications, global download and upload limits. Speed is unlimited by default; the minimum enabled limit is 0.02 MB/s. Seeding continues after downloading; "Pause" stops it. The author's MIT license, dependency licenses and the Slint badge are kept. Do not remove the LSD fix for Mac.

```powershell
cargo fmt --all -- --check
cargo clippy --locked --all-targets -j 2 -- -D warnings
cargo test --locked --all-targets -j 2
cargo build --locked --release -j 2
```

Cargo on the current machine: `C:\Users\Engus\.cargo\bin`. The latest local all-targets runs were stopped by Windows App Control with code 4551 before tests ran; a retry without changes was blocked too. Protection/ACLs were not changed and the block was not bypassed. The complete final suite is confirmed by CI; local fmt/check/Clippy and release builds passed.

Occasionally the upstream Slint build script gets PermissionDenied when overwriting `target/{debug,release}/build/SLINT_DEFAULT_STYLE.txt`. After Cargo has fully finished, it is fine to delete only that verified six-byte generated file containing `fluent` and rerun the same command. Do not change ACLs or system protection. Limit local parallelism to two jobs; the earlier linking overload and Git recovery are described in the history, and Git is currently healthy.
