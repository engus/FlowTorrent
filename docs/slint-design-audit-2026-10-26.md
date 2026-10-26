[Русский](slint-design-audit-2026-10-26-ru.md)
# Native Slint design audit — 26 October 2026

Source `15a1be13baf93245a8c5fcf0f0a066fb239df713`, branch `codex/slint-release-readiness`, reviewed against base `3936d0a`. The reference is the approved v44 white-background/bottom-drawer mockup under `.superpowers/brainstorm/71364-1792932748/content/`. The audit compares the real Slint interface, not an HTML implementation presented as the application. AppKit, engine, settings schema, dependency versions and the installed app are unchanged. No release, tag or installer upload is authorized by this work.

## Findings and corrections

| Area | Finding | Correction |
| --- | --- | --- |
| Main table | Narrower size/state/progress columns, larger row-name type and taller header changed the balance of the approved layout | Keep 64px rows; use a 40px header, 12px names and 88/132/240px wide size/state/progress columns. Responsive widths retain space for names and selection controls |
| Row actions and state | Boxed ellipsis and a second status dot added visual noise | Flat ellipsis; retain the leading colored state icon and explicit status text |
| Status bar | Full-window footer interrupted the sidebar; arrow icons were not vertically aligned with rate labels | Footer belongs to the right content column; align icons and rates centrally in a 34px strip |
| Theme setting | Dropdown differed from the approved segmented control | Three accessible radio choices with one Tab stop; Left/Right/Home/End navigation and existing asynchronous autosave |
| General settings | Left checkboxes differed from the mockup's right switches | Whole-label clickable switches, accessible checked state, Space/Enter control, disabled during a pending transaction |
| Settings proportions | Larger labels/selects and an expanded About button dominated the form | 12px labels, 175px language/rate selects and a natural-width About button |
| Settings failure | Backend error at the end of a scrolling form could be missed | Pin the error above the form. Failed changes still restore the durable saved values and focus |
| Drawer files | Fixed metadata widths could squeeze file names at intermediate widths | Combine file icon/name in one flexible cell, adapt bar/metadata widths and stack metadata on narrow screens. Keep actions/search pinned |
| Category wording | Incomplete/Downloaded differed from the requested clearer labels | All / Downloading / Completed; Все / Загрузки / Завершённые |
| Attribution placement | A second plain footer line repeated the official About attribution | Retain the official AboutSlint widget in About, reachable from the native top-level Help menu and Settings; remove the redundant footer text |

The palette remains white for the light primary surface and neutral gray for secondary/dark surfaces. Existing Nunito, brand assets and Lucide icons are retained. Progress stays left-aligned. Global Start all/Stop all remain registry-wide; row ellipsis opens the real bottom drawer. None of these audit corrections introduces an extra settings write on opening the pane or changes the file/Trash/transfer commands.

## Wording and attribution

**Completed** is the intended English category adjective. Downloading continues to include every unfinished torrent, including stopped, waiting and error states; Completed includes seeding. Renaming does not hide stopped/error records. The behavioral regression covers these memberships. Downloaded remains appropriate for the amount already received and individual file states; those are not category names.

[Slint Royalty-free 2.0, section 2(a)](https://slint.dev/agreements/slint-royalty-free-license.pdf) specifies an official AboutSlint widget in an About dialog accessible from a top-level menu as one attribution option. The current app implements that option. The official widget/badge, dependency notices and MIT license stay intact. A duplicate plain “Made with Slint” line in the main window's bottom-right corner is unnecessary for this chosen placement. This does not remove attribution requirements for future distribution or website changes.

## Native observations and boundaries

macOS 27.0.1 / Apple Silicon / built-in Retina, Winit software renderer, owned real-engine fixture `/private/tmp/flowtorrent-slint-design-audit-20261026`. It contains two completed records (one with 40 files), a stopped zero-progress record with a long name and an unavailable-base-folder error. The normal user's registry and paused NAS torrent were not used. Public trackers/DHT are disabled by the harness; notifications and launch at login remain off at the end.

- Compared dark RU Settings and dark EN main/drawer at 1280 × 720 client size; light RU main/drawer/Settings/About at 800 × 640 with maximum 280px sidebar. File names, stacked file metadata, pinned actions and white background remain usable. Initial background-window capture was clipped until raising the test window; the corrected screenshot is the recorded compact evidence.
- Theme mouse selection, Right/Home/End keyboard navigation, persisted values and focus after successful save passed. Notification switch mouse-on / Space-off passed using the harness's no-op notification sink; no OS permission was changed. Launch at login was not enabled.
- With only the fixture's default folder made unavailable while the app was stopped, a theme change failed, kept saved Light, restored keyboard control and displayed the localized error above the form. The original fixture settings were restored after shutdown.
- The official MadeWithSlint widget remained visible in compact light About. The native Help entry remains in the platform adapter; existing headless About lifecycle tests ran in the complete suite. This task did not repeat the previous browser-link/payment or physical Finder/Dock checks.
- Add/preview/magnet/error/busy paths retain their existing callbacks and dialogs; source review and the full regression suite cover the integration. No fresh complete manual dialog matrix is claimed.

The browser reference used width 1280, but its host viewport and demonstration/title strips differ from the native client height. Screenshots are qualitative evidence backed by explicit row/column dimensions, not a pixel-identical image claim. Real paths, unknown progress shown as “—”, actual file sizes, session-scoped upload percentage and the added About dialog deliberately differ from demonstration data. Windows physical rendering, keyboard/DPI/screen-reader acceptance and supported-system/Intel physical checks remain open.

Local screenshots/logs are under `target/slint-design-audit-20261026/`: `mockup-main-before.png`, `mockup-settings-before.png`, `mockup-drawer.png`, `native-main-before.png`, `native-settings-before.png`, `native-main-final.png`, `native-drawer-final.png`, `native-compact-drawer.png`, `native-settings-failure.png`. They are ignored test artifacts rather than repository assets.

## Verification

The missing category-label function was first reproduced by a failing test, then the labels/membership test passed. Formatting, strict all-target Clippy, **138 Rust tests across 33 targets (0 failures/ignored)** and **43 Python tests** passed on pinned Rust 1.92.0, locked dependencies, `-j 2`, macOS deployment target 14.0. Independent read-only review of `3936d0a..15a1be1` found no Critical/Important issues; physical rendering and licensing were explicitly outside that review and are evidenced separately above.

ARM release and the macOS lifecycle probe passed. The release uses the previously documented host build-tool workaround, `--config 'profile.release.build-override.strip="none"'`; the application retains its optimized release profile. Logs: `release.log`, `lifecycle.log`.

Local test bundle: `target/local-macos/slint-design-audit-20261026-15a1be1/FlowTorrent.app`, ARM64/macOS 14+, ad-hoc signed without Developer ID/notarization. Plist, strict signature and complete Lucide notice verification passed. Executable **31,576,768 bytes**, bundle **37,307,254 bytes**; SHA-256 `afc9fcc988db2a2e1fdd49d8db75c21e40ad50db6ff6aa12b6e8a03d3f53d3ee`. `build-evidence.json` sits beside the app. Quit an older ordinary FlowTorrent before opening this candidate; the installed application was not replaced.

[Check 40335015994](https://github.com/engus/FlowTorrent/actions/runs/40335015994) **SUCCESS**, exact source `15a1be1`: Windows x64, Mac ARM, Mac Intel and dependency policy passed. Each platform passed formatting, Python packaging tests, all-target check/Clippy/Rust tests and bounded bencode checks; both Macs passed the lifecycle probe. `test-packages` was **SKIPPED**. The only uploaded artifact is `dependency-policy-inventory` (344,856 bytes); no installer uploads, release or tag. Physical release gates remain open.
