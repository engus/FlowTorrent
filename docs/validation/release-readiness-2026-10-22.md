# Slint release preparation — 22 October 2026

This is the historical local-packaging snapshot. Subsequent authorized branch
pushes and native CI results are recorded in
[the 23 October follow-up](windows-validation-2026-10-23.md), which supersedes
the pending native-validation statements below.

The first packaging batch is implemented and locally verified. **1.0 is not
release-ready.** Packages retain Cargo version **0.1.0**, are explicitly local
test builds, and are ad-hoc signed on Mac. No release, tag, source push or
installer upload was performed.

Work is on `codex/slint-release-readiness`, isolated from the original checkout's
uncommitted documentation and the AppKit experiment. Production base `91872e0`
already includes the audit follow-up and memory fixes. This batch changes no
Rust application source, Slint UI, Cargo dependencies or vendored engine code.
The previously delivered AppKit executable still has SHA-256
`68d8a0c9aeb05b5f0659e4bc0159bff6dc1b5df0d8aa8795e242e7ff0798d74a`.

## Implemented and verified locally

- Mac ARM64 and Intel app/DMG packaging, test-build instructions, SHA-256 and
  provenance manifests. Both DMGs were mounted read-only; the mounted bundles
  passed strict signature verification, architecture/deployment/dynamic-library
  checks, version/identity checks, Viewer/Alternate file registration checks and
  complete notice-inventory verification. Each was detached after verification.
- Exact locked target dependency notices, supplemental copyright/license files,
  Rust standard-library notices and unchanged MPL component source. Coverage:
  ARM64 447 crates / 773 assets; Intel 450 / 778; Windows 439 / 781.
- Windows unsigned per-user Inno Setup implementation, icon/version resources,
  quoted Open With registration and Start menu notification identity. No default
  handler takeover or wildcard uninstall cleanup. English/Russian test notices.
  **Native Windows compilation and installation have not been run.**
- Manual/reusable three-platform test-package workflow; installer uploads default
  to false. The disposable Windows install/reinstall/uninstall test checks state,
  payload and unowned-file preservation. It is prepared, not executed evidence.

The packaging review found two important defects. Regressions reproduced the
unlisted-file/symlink notice bypass before fixing it. Collection now rejects
unlisted files and links, including a linked manifest. Inno's numeric version
default was confirmed in the official documentation; setup now sets both numeric
versions from Cargo, and native verification checks numeric and textual fields
on the app and installer. That Windows verification awaits a native run.
The minor English-only Russian installer notice was also corrected.

| Local check | Result |
| --- | --- |
| Rust 1.92, `cargo test --locked --all-targets -j 2` | 112 passing records across 28 executables; zero failed/ignored |
| `cargo clippy --locked --all-targets -j 2 -- -D warnings` | Passed |
| `cargo fmt --all -- --check` | Passed |
| Python packaging/policy suite | 35 passed |
| Native Slint/Winit lifecycle probe | Passed: delegate identity, ordered file callbacks, reopen callback, hidden event loop |
| Workflow YAML parsing | Passed; GitHub Actions execution remains pending |
| ARM64 Mac package GUI | Passed bounded smoke below |
| Intel Mac package | Cross-built and inspected; not executed on physical Intel hardware |

The final ARM64 app launched from its delivered bundle. The native Add torrent
picker opened and cancelled, the empty magnet submission was disabled, malformed
input was rejected, and Settings opened/cancelled with Tab focus navigation.
Opening a generated local `.torrent` through Launch Services reached the preview
in one existing process; cancellation created no payload. Command-Q displayed
shutdown progress and the process exited. Relaunch returned to the empty list.
The new ARM64 test app is left open. These checks do not establish a clean-system
installation, Finder/Dock acceptance, notifications or sleep/wake behavior.

## Local deliverables

Source/package revision: `2dba9f6c24f9bfdb61afff565e4ce9e6c7b660df` (clean).
Later documentation commits do not change these package inputs.
[Machine-readable package evidence](release-packages-2026-10-22.json) records
exact bytes, hashes and notice counts. Sizes below use MiB (1,048,576 bytes).

| Target | Executable | App files | Compressed DMG |
| --- | ---: | ---: | ---: |
| Mac ARM64 | 25.16 MiB | 30.29 MiB | 14.57 MiB |
| Mac Intel | 27.70 MiB | 32.85 MiB | 15.77 MiB |

Copies are in the original project's ignored
`target/slint-1.0-test-packages/mac-arm64/` and `mac-intel/` folders, with the app,
DMG, checksum and manifest together. Copied artifact hashes and bundle signatures
were verified. Validation logs are alongside them in `validation-logs/`.
Existing `target/local-macos/FlowTorrent.app` and AppKit artifacts were preserved.

Host: Apple M3 Pro / 18 GiB, macOS 27.0.1, Xcode 27.1; pinned Rust 1.92.0,
`MACOSX_DEPLOYMENT_TARGET=14.0`, local Cargo parallelism 2. Build metadata is not
proof of execution on macOS 14.

## Remaining order of work

1. Run the prepared native three-platform CI and Windows disposable installer
   check. The current `gh` OAuth token lacks the `workflow` scope needed to push
   these workflow edits; no credential permissions were expanded. A maintainer
   can authorize workflow access, then push this branch and dispatch `Check`
   with `test_packages=true`. Installer uploads should stay disabled.
2. Check clean Windows 10/11 and macOS 14+ installs, actual file opening while
   hidden, full keyboard/dialog workflows, tray/Finder/Dock, physical DPI/Retina
   and notification delivery/denial. Perform a real cross-version upgrade with
   session restore, then uninstall while preserving downloaded files.
3. Complete overnight controlled download/seeding, forced-exit recovery and
   real sleep/wake on both systems. Bounded probes do not close these gates.
4. Review native-runtime redistribution notices, complete signing/notarization
   when credentials exist, rerun installed-package acceptance and promote the
   version only after the [release checklist](release-checklist.md) passes.
   Public release remains a separate maintainer action.

Build commands and workflow details: [test packages](../test-packages.md).
