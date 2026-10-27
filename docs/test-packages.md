# Slint 1.0 test packages

This continues release-plan task 10 using the existing Rust/Slint application.
Packages remain **0.1.0 local test builds** until the release checklist passes.
No release, tag, installer upload or default-application change is automatic.
AppKit experiments are outside this branch.

The [22 October local report](validation/release-readiness-2026-10-22.md) records completed
Mac packages, actual sizes, verification and the still-open release gates.
The [native CI follow-up](validation/windows-validation-2026-10-23.md) records successful
three-platform core checks, native packaging and the disposable Windows
install/reinstall/uninstall preservation test. Physical acceptance remains open.

## Reproducible notices

Use the pinned Rust 1.92 toolchain and keep its runtime copyright/license files.
Install the notices generator separately from application dependencies:

```sh
cargo install cargo-about --version 0.9.2 --features cli --locked -j 2
python3 scripts/generate_notices.py --target aarch64-apple-darwin --output target/notices/mac-arm64
```

Use `x86_64-apple-darwin` or `x86_64-pc-windows-msvc` for the other targets.
Choose a fresh output directory. Collection uses locked normal/build dependencies,
supplemental license/NOTICE files, Rust standard-library notices and unchanged MPL
component source. The manifest fingerprints Cargo.lock, the policy and every file.
Packaging refuses stale, altered or wrong-target notices.

`cargo-about 0.9.2` can exit zero while omitting clarified `LicenseRef` text.
The wrapper checks coverage for every resolved crate and supplies Slint 1.18.1's
royalty-free text only when its locked-source SHA-256 matches the reviewed value.
It never substitutes an empty section or a generic MIT copyright. Collection of
this workspace itself is limited to its own LICENSE; build outputs and user data
are not scanned. Generated HTML escapes dependency content.

## Mac ARM64 and Intel

```sh
./scripts/package-macos.sh --target aarch64-apple-darwin \
  --notices target/notices/mac-arm64 --dmg \
  --output target/test-packages/mac-arm64/FlowTorrent.app
```

For Intel, install the matching Rust target and generate its notices first:

```sh
rustup target add x86_64-apple-darwin
python3 scripts/generate_notices.py --target x86_64-apple-darwin --output target/notices/mac-intel
./scripts/package-macos.sh --target x86_64-apple-darwin \
  --notices target/notices/mac-intel --dmg \
  --output target/test-packages/mac-intel/FlowTorrent.app
```

The bundle and executable are checked for architecture, macOS 14.0 support,
system-only dynamic dependencies, matching Cargo/bundle versions and ad-hoc
signature integrity. Notices are included before signing. The verified compressed
DMG contains the app, an Applications link and test-build instructions. Its
`.dmg.sha256` and `.dmg.json` record bytes, hash, source revision and lock hash.
Existing images/evidence are never overwritten. A dirty source tree is identified
in the artifact manifest. Ordinary `.app` development without a DMG still works.

Ad-hoc signing is for local testing. Developer ID signing, notarization and
clean-system Gatekeeper/installation checks remain separate release gates.
Do not disable OS security protections to run a test build.

## Windows x64

Build on Windows using Visual Studio C++ tools, Windows SDK and Inno Setup 6.3+
(the wrapper locates `rc.exe` and `ISCC.exe`, or accepts `-Iscc` explicitly).

```powershell
python scripts/generate_notices.py --target x86_64-pc-windows-msvc --output target/notices/windows-x64
./scripts/package-windows.ps1 -Notices target/notices/windows-x64 -Output target/test-packages/windows-x64
```

The packager embeds the existing project icon and a version resource matching
Cargo, uses the static CRT for this packaged target, and verifies AMD64 PE identity
and native numeric/textual version properties on both the executable and setup.
The installer is unsigned and labeled
as a test build. It installs per-user to LocalAppData/Programs/FlowTorrent without
administrator privileges, with a Start menu shortcut using `com.engus.flowtorrent`.
The setup and payload include the project license, generated notices and Inno's
license. Artifact hashes/manifests are saved next to the installer.

`.torrent` registration adds `FlowTorrent.Torrent` as an Open With/default-app
candidate, with quoted executable/file paths. It does not write the extension's
default value or protected UserChoice keys. System-wide magnet registration is
unchanged. Updating/removing the program must preserve the separate session,
settings and payload locations. There is no wildcard uninstall cleanup.

`scripts/tests/test_installer_windows.ps1` is only for a **disposable GitHub
Windows runner**, guarded by `-DisposableProfile`, an Actions environment and
absence of existing FlowTorrent state. It checks a silent per-user install,
same-version reinstall, shell/shortcut identity, unchanged default handler,
state/payload hashes and preservation of an unowned file during uninstall. It
does not claim real UI, cross-version session restore or Windows 10/11 physical
validation. Do not run it in your ordinary Windows profile.

## CI and publication boundary

`Slint test packages` is manual/reusable. Its artifact upload input defaults to
false. It checks dependency policy, portable tests, Rust tests/Clippy, notices
and native packaging on Windows x64, Mac ARM64 and Mac Intel. Windows then runs
the disposable installer check. There are no release/tag triggers, release API
calls, write-content permissions or automatic installer uploads.

Before this new workflow reaches the default branch, use the existing `Check`
workflow on the development ref with `test_packages=true`; it calls the reusable
workflow from that same ref, with uploads disabled. After review, a maintainer
can explicitly request saving test artifacts. Building/uploading test artifacts
does not authorize publishing a GitHub Release.

Version promotion, signing/notarization, native-runtime notice review, clean
install/cross-version upgrade, sleep/wake, overnight operation, notification
delivery/denial, accessibility and physical display checks remain gated by
[the release checklist](validation/release-checklist.md).
