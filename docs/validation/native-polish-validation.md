[Русский](native-polish-validation-ru.md)
# Native UI corrections — 27 October 2026

Work is isolated on `codex/slint-native-polish-20261027`, based on `fdd209e`. The primary checkout and AppKit prototype are unchanged. No release, tag or installer upload is authorized by this work.

## Physical Mac evidence

The ordinary application's menu-bar preference was turned OFF, the app quit gracefully and the same bundle was restarted. Settings still showed OFF. It was restored to ON; the remaining preferences and torrent desired states were preserved. This was an actual restart, not a serialization-only test.

Further tests use an independently identified, ad-hoc-signed `FlowTorrentPolishAudit.app` and owned `/private/tmp/flowtorrent-native-polish-20261027` registry with a loopback-only seeder. The user's active transfers in the ordinary app are not used as fixtures.

- Native add sheet: `.torrent` picker → preview → selected files → download → completed local payload passed. Cancel/Escape and folder-picker Cancel returned to a usable parent.
- Duplicate magnet for the existing fixture closed the child, focused its existing row and left one registry entry.
- Light/Russian and dark/English sheets inspected. A 100-file preview scrolled to `file-099.txt` while Cancel/Download stayed visible; cancellation added no record.
- The native lifecycle probe attached/detached a real document sheet twice and preserved the existing hidden-window event loop checks.
- Native Stop changed Seeding to paused/Completed; Start returned it to Seeding. Tab and Enter also stopped it. Remove only opened the correct confirmation; Cancel kept the torrent and payload. The initial failure was confirmed by the user: closing the popup destroyed its weak callback-parent chain. A rendered headless pointer test reproduced it; dispatching the action before closing passes. Speculative size/focus changes were reverted.
- Payload SHA-256 matched `106eda6faccdbfbe8fb0dfa8f793fdacafda3f3904fafd3f0f7c5932b2a4c090`. Cancelled previews added no records or payloads.

## Implementation boundaries

The Mac status image uses native template tint, adapting to the system menu background. The Dock icon and Windows tray stay colored. Mac transfer statistics occupy one stable menu item; its detailed tooltip is retained. A 24px brand mark replaces the expanded sidebar heading.

Adding uses an owned native window (AppKit document sheet / Win32 owner with the main window disabled). Native handles attach on readiness events; no polling is added. Leases retain neither Winit `Arc` nor Slint `WindowHandle`, and release before hiding either window. Preview-instance revisions reject callbacks from a cancelled/requeued preview with the same torrent hash. Native creation failure keeps a Cancel/Retry route.

## Automated verification

Source `f84b369` passes local fmt, strict all-target Clippy and all-target Rust tests: 156 passed, zero failures/ignored across 35 binaries. The final native lifecycle probe passes. Python: 45 passed; 177 contextual messages in 14 languages validated. One fresh read-only review reported no Critical/Important findings; the subsequent user-confirmed popup bug was fixed with RED→GREEN and the full suite.

[Three-platform CI](https://github.com/engus/FlowTorrent/actions/runs/40496703230) passed on this source commit: Windows x64, Mac ARM and Mac Intel, including strict Clippy and all-target tests. Both Mac native lifecycle probes passed; dependency policy passed. Package jobs were skipped; no installers were uploaded. No Windows physical-runtime acceptance is inferred from Mac testing or CI.

The optimized ARM test package is `target/local-macos/slint-native-polish-20261027-f84b369/FlowTorrent.app` (38 MiB on disk). Strict ad-hoc signature and lockfile-specific notices passed. Packaging uses the existing macOS 27 host build-override strip workaround; the application stays optimized/stripped. It is not Developer ID signed or notarized. The diagnostic app was stopped and this ordinary bundle launched with the user's two saved records; the menu-bar switch remained ON. No installed bundle was overwritten.
