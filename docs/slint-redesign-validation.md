[Русский](slint-redesign-validation-ru.md)
# Slint redesign candidate — 26 October 2026

The approved design is connected to AppController/librqbit in the shared Slint UI. This is a local development candidate, not a public 1.0 release. AppKit prototype sources and the existing installed app were not changed.

Implemented: brand SVGs/icons and Nunito; All/Incomplete/Downloaded; name search and sorting; selection by immutable ID; global Start all/Stop all; row menus; real rates, ETA and session upload percentage; per-file checked bytes, folder folding/search and pinned actions. Ordinary selection does not show checkboxes. Full file details are requested only for the inspected visible torrent, retaining hidden-window update coalescing.

Preferences include System/Light/Dark, RU/EN, opt-in login, folder for new downloads, notifications and global rate presets/custom values. Defaults are Unlimited. New minimum: 0.5 Mbit/s = 62,500 B/s, displayed exactly as 62.5 KB/s. Old valid limits remain unchanged. Cancel restores saved appearance/language. Winit theme events update System mode on both platforms without overriding explicit Light/Dark.

Remove only keeps files. Remove with files uses OS Trash, explicitly approved by the user. Confirmation captures IDs/names and focuses Cancel. SecureStorage deletion stays disabled. A separate operation persists stop intent, releases the engine, validates pinned regular files, rejects replacements/links, stages selected owned files in a private same-volume folder and journals the native operation. Receipts remain until registry removal is durable. Failed/interrupted operations preserve data and block automatic payload recreation. Unrelated files and empty original folders remain.

## Evidence

- Rust 1.92.0, --locked, -j 2, MACOSX_DEPLOYMENT_TARGET=14.0.
- fmt, all-target Clippy with warnings denied, all-target tests: 130 tests across 33 completed targets, no failures.
- ARM release build and Mac lifecycle probe passed. The established macOS host build-override.strip=none workaround was used for proc-macro compatibility; executable optimization and Windows profiles are unchanged.
- Intel Mac cross-target cargo check passed; no physical Intel execution is claimed.
- Windows dependencies and actual native login/Trash/staging modules were type-checked against Windows Rust std and Win32 bindings. The probe stubs unrelated domain/persistence helpers; it is not a complete Windows app build or CI run.
- 40 packaging Python tests passed. Plist, Mach-O architecture/minimum OS/system dependencies, ad-hoc signature and notices matching unchanged Cargo.lock were verified. Nunito OFL is included separately.
- Regression tests cover selection/filter reconciliation, real/hidden details, repeated bulk actions, paused Trash, replacements/links, unexpected staging entries, failed/interrupted Trash, stale add receipts, old settings, exact units, cancellation and rollback to the actual previous login entry after persistence failure. Fresh review findings were reproduced and fixed.

## Physical Mac checks and limits

The real UI used an isolated registry and loopback-only seeder. A 524,288-byte fixture downloaded, showed verified completion and stopped through its row menu. Native Trash reported success for that stopped fixture: its original path and registry row disappeared and the active journal was removed. Terminal inspection of ~/.Trash was denied by macOS privacy; restoration from Trash was not tested. Trash was never emptied.

A 40-file local fixture exercised the system picker, real preview/confirmation, waiting/stopped states, folder folding, searching folded groups and restoring folding, pinned actions, torrent search/empty search and ordinary selection without checkboxes. Light/dark, RU/EN, Unlimited, minimum preset and Cancel were exercised. A missing modal focus was reproduced and fixed; Cancel focus and Escape then passed. Screenshot: target/slint-redesign-validation-20261026/native-light-files.png (before renaming Paused to Stopped).

The regular user registry/payload was not used for transfer/Trash tests; its paused NAS torrent was not started or removed. Login entries were not enabled during validation. Full CI, physical Windows DPI/keyboard/Recycle Bin/login, physical Intel execution and real login/logout remain release gates. No fresh CPU/RAM benchmark is claimed. Upload percentage is explicitly session-scoped; lifetime persistence is not implemented. Folder relocation remains outside this transfer; Retry rechecks the original path.

## Candidate and handoff

Branch: codex/slint-release-readiness. App: target/local-macos/slint-redesign-20261026/FlowTorrent.app, arm64, macOS 14+. Ad-hoc signed, no Developer ID or notarization. LOCAL_BUILD.txt labels it; build evidence and SHA-256 sit beside it. The regular app uses the regular registry when run; the fixture harness used a separate one.

Keep this branch/worktree for iteration. Next gates: full Windows/Mac CI and the physical checks above. No release, tag, installer upload, push or PR was created by this task.
