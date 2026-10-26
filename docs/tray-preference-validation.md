[Русский](tray-preference-validation-ru.md) · [Documentation](README.md)

# Tray preference and Mac background icon — 26 October 2026

The approved v45 setting is now in the shared Slint settings pane. It defaults to enabled for existing settings and saves automatically. macOS calls it **Show icon in the menu bar**; Windows uses **Show tray icon**. It controls only the notification-area/menu-bar icon, leaving the Mac Dock icon and background transfers available. All fourteen language catalogs include the control and guidance.

## Implementation and regression coverage

The Mac icon now has an independent native `NSStatusItem`, retained action target and stable `NSMenu`, owned by the platform shell rather than a Slint tray window. Rates and counts update existing menu items in place. The main window can hide without destroying this owner. Explicit disable and process exit remove it; enabling creates a single owner with current language and statistics. No recovery polling or extra periodic timer was introduced. The brand SVG is rasterized at 36 pixels for an 18-point Retina icon.

This is a platform workaround verified against the observed regression. The precise upstream Slint trigger has not been established. Windows retains its Slint tray implementation and gains **File → Quit**, ensuring exit remains available with the tray disabled; reopening by launching FlowTorrent uses the existing single-instance path. The AppKit UI prototype is untouched.

Tests cover old/partial settings defaulting on, false roundtripping, actual UI save/rollback/focus, unrelated edits preserving false, and preference changes waking a hidden UI while disabled transfer-rate changes do not. A review caught a damaged/future session preventing valid settings from loading: settings now load first, and a regression verifies disabled intent and both files’ original bytes survive. The native lifecycle probe checks lazy startup, disable/enable, hide, terminal shutdown, delegate and hidden event-loop behavior. The Windows Quit callback has queue-full retry and duplicate-shutdown regressions.

## Physical Mac evidence

Only the owned `FlowTorrentTrayPreferenceAudit.app` and `/private/tmp/flowtorrent-tray-preference-20261026` fixture were used on Apple Silicon, macOS 27.0.1 (26A434). The normal installed application, user torrents and primary dirty checkout were preserved. This bundle has a real Mach-O executable and local ad-hoc signature; it is a test harness, not an installer or signed release.

- Native settings: switch off/on, focus retention and exact saved JSON false/true verified. Light Russian Retina screenshot: local `target/slint-tray-preference-20261026/native-settings-light-ru.png`.
- The full window was closed during a rate-limited, loopback-only 16 MiB download. The maintainer confirmed the upper menu-bar logo remained and its menu worked. This is the scenario that previously lost the icon.
- The hidden download completed with all 16,777,216 bytes matching the controlled source; SHA-256 `3f5acf06f1ec4128a69d246cec655c8d82ceb48b98df79e4ba69affafd80c146`. The fixture was then stopped before further lifecycle checks.

After the final restart and another hide, the maintainer again confirmed the icon remained and Show FlowTorrent reopened the window. Saved-state restart and final local automated results are recorded below. This bounded Mac observation does not prove overnight stability, physical Intel behavior, Windows tray/DPI rendering or every OS version. Cross-platform CI and the remaining physical release matrix remain separate evidence. No release, tag or installer upload is authorized by this report.

## Final automated checks

At source `5fe87cb`: all-target Rust tests **149 passed, 0 failed, 0 ignored across 35 targets**; Clippy with warnings denied passed; fmt passed; Python tests **45 passed**; generated catalogs **176 contextual messages / 14 languages**. The Mac lifecycle probe passed on the main thread. Focused review fixes had reproducing failures before implementation and passed afterward. Cargo.lock and pinned crate versions are unchanged.

A real native restart restored the saved switch **off**, then a second restart restored **on** after an automatic save. The registry remained paused throughout these restarts. [Final three-platform core CI](https://github.com/engus/FlowTorrent/actions/runs/40367260566) checks code-identical documentation head `de9f8c9`. All three targets and dependency policy passed: Windows x64 **147 tests / 35 targets**, Mac ARM and Intel **149 / 35 each**, plus **18 bencode tests** on every target and lifecycle probes on both Macs. No failures or ignored tests. The first ARM job was cancelled before acquiring a hosted runner; GitHub annotations identified capacity constraints. Retrying only that job succeeded in attempt 2. Test-packages is skipped and installer uploads remain disabled. The earlier source run was superseded by this final push. CI does not establish physical Windows/Intel tray behavior.

## Local candidate

The optimized ARM application was packaged successfully at local `target/local-macos/slint-tray-preference-20261026-5fe87cb/FlowTorrent.app` (about 38 MiB on disk), with target-specific notices verified against the unchanged lockfile. Strict ad-hoc signature verification passed. The packaging command retains the documented host-build strip workaround for macOS 27; this is not evidence that an unmodified `cargo build --release` command works on that host. The installed app was not replaced, and no DMG/installer/release was uploaded.
