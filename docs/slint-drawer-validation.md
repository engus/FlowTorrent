[Русский](slint-drawer-validation-ru.md)
# Native Slint bottom drawer validation — 26 October 2026

Source: `b00b27dbd03c3878d74d296e56da593ed2c64ea5`, branch `codex/slint-release-readiness`. Implements the [approved drawer spec](superpowers/specs/2026-10-26-slint-drawer.md) and its [plan](superpowers/plans/2026-10-26-slint-drawer.md). This follows the historical [parity validation](slint-parity-validation.md).

## Result and scope

The shared Mac/Windows Slint UI now uses a white light primary background, the approved neutral dark palette, static Lucide control/status icons, a collapsible/resizable left sidebar without the large logo, and an explicit bottom details drawer. The drawer pins actions and file search while independently scrolling information and files. Top Start all/Stop all remain global; the selection toolbar's Actions opens bulk controls without discarding selected IDs. Ordinary selection leaves a closed drawer closed.

Settings apply through the existing durable asynchronous transaction without Save/Cancel or manual rate entry. Conflicting edits wait for completion. Failure restores saved controls/appearance/layout; initiating form/separator focus survives completion. Unrelated edits preserve exact older non-preset limits and login preferences. UI layout uses compatible defaults in saved preferences. No payload, engine, IPC, notification or Trash redesign; no AppKit prototype changes. No release, tag, installer upload or installed-app replacement.

## Automated evidence

All commands use Rust 1.92.0, the locked graph, local `-j 2`, and deployment target 14.0. Ignored raw logs are under `target/slint-parity-validation/`.

- `cargo fmt --all -- --check`: passed (`drawer-fmt-final.log`).
- `cargo clippy --locked --all-targets -j 2 -- -D warnings`: passed (`drawer-clippy-final.log`).
- `cargo test --locked --all-targets -j 2`: **135 passed, 0 failed, 0 ignored**, 33 targets (`drawer-tests-final.log`).
- `python3 -m unittest discover -s scripts/tests -v`: **43 passed**, including complete icon/font notices, real Mac bundle inclusion and the Windows installer Files entry (`drawer-python-final.log`).
- ARM release and native lifecycle: passed (`drawer-release-target-final-unstripped-host.log`, `drawer-lifecycle-final.log`).
- Three-platform Check: [Check 40312442344](https://github.com/engus/FlowTorrent/actions/runs/40312442344) passed on Windows x64, Mac ARM and Mac Intel at the exact source SHA above, including fmt, packaging scripts, all-target check/strict Clippy/tests, bounded bencode tests and both Mac lifecycle probes. Dependency policy passed. `test-packages` was skipped on this push; no installers were built/uploaded by this run. Only the dependency inventory artifact was uploaded.

Focused RED→GREEN regressions cover automatic submission/pane retention/failure restoration, exact 20,000 B/s legacy limit retention, explicit inspection and closure, layout defaults/bounds/round trip, in-progress resize protection from transfer snapshots, failed layout restoration and focus-ticket restoration. Final review found inaccessible bulk actions from a closed drawer and a missing Inno Setup Lucide notice entry; both have reproducing regressions and fixes. No Critical or other Important findings were reported. The independent review covered `edf22de..d8e1f59`; fixes landed in `d2cd58e`. The final parent icon scan also replaced the old preview disclosure glyph in `b00b27d`, with native expand/collapse verification (`native-lucide-preview.png`).

Release build environment note: this Mac's loader rejects the stripped build-time `zerofrom_derive` dylib with `mis-aligned LINKEDIT string pool`; the minimal extern probe reproduces it while the debug dylib loads. Cleaning that generated package and changing only the target did not fix it. The successful build uses `cargo build --locked --release --target aarch64-apple-darwin -j 2 --config 'profile.release.build-override.strip="none"'`: only build-time tools retain their symbols; the application keeps its normal optimized/stripped release profile. No toolchain/dependency upgrade or system protection change was used.

## Physical Mac UI with controlled engine data

`examples/ui_smoke` used its own registry and payload at `/private/tmp/flowtorrent-slint-drawer-20261026`, a loopback-only seeder, notifications/login disabled. The normal user's registry, paused NAS torrent and installed application were not used.

- Native compact 800 × 640 window at Retina 2×: white light and neutral dark, RU/EN; settings controls and pinned drawer actions fit with sidebar expanded 280. File rows stack when narrow rather than compressing names to an ellipsis. Screenshots: `target/slint-drawer-validation/native-white-ru-compact.png`, `native-dark-ru-compact.png`, `native-white-compact-drawer.png`.
- Ordinary row click left drawer closed/no checkboxes; ellipsis opened real 40-file details; query 40 returned `File 40.bin` while action/search areas stayed pinned. Sidebar collapsed 56 and expanded 158–280; keyboard Home/Right/End and drawer-height keyboard adjustment worked and retained separator focus after saving. Layout survived relaunch.
- Theme, language and minimum 0.5 Mbit/s preset saved immediately, retained pane and initiating focus. The UI showed no Save or custom rate entry. A controlled unavailable default folder rejected a theme change, restored Light and focus, left JSON unchanged, and displayed the localized folder error when the compact form was scrolled down.
- Native preview confirmed a new controlled torrent; all 524,288 downloaded bytes matched `tests/support::payload`, SHA-256 `106eda6faccdbfbe8fb0dfa8f793fdacafda3f3904fafd3f0f7c5932b2a4c090`. Stop→Completed and Start→Seeding were observed after completion; this is not evidence of pausing a partially downloaded file. Both records were stopped after checks.
- Search checkboxes selected both records with drawer initially closed. Actions opened the bulk drawer preserving count 2, showing no aggregated file details; group Start/Stop worked. Headless regression also verifies no single-torrent InspectDetails request in this path.

## Local application and boundaries

`target/local-macos/slint-drawer-20261026-b00b27d/FlowTorrent.app`, ARM64/macOS 14+, source `b00b27d`. Local test build, ad-hoc signed; no Developer ID/notarization. `plutil -lint`, strict `codesign --verify` and exact Lucide notice comparison passed. Executable 30,850,256 bytes; bundle 36,580,725 bytes. Executable SHA-256 `6090ab7b5bd29281869caf609be4ae3c9a735368d17ee84f5bc0d29ee88eecda`. Build evidence is beside the app; package log: `drawer-package-final.log`. Quit any running ordinary FlowTorrent before launching this candidate: the normal app uses its normal registry and single-instance routing.

Lucide's 32 SVGs and complete ISC/Feather MIT notice match official pinned revision `05232756c05e9ca7d5ad267e8694e9afa5d572fa`; provenance and hashes live under `assets/icons/lucide`. Both package builders use the same complete asset-notice helper; Inno Setup includes the root icon notice explicitly. Nunito OFL, author MIT, locked dependency notices and visible Made with Slint remain.

Physical Windows rendering/DPI/keyboard/Recycle Bin/login and physical Intel execution remain manual acceptance gates. This pass does not re-claim earlier Dock/sleep/notifications checks for the changed layout, test real login/logout, or provide a new CPU/RAM benchmark. Existing long-running release gates remain open. CI does not replace those physical checks.
