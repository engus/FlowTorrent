[Русский](2026-10-27-native-polish-ru.md)
# Native UI polish Implementation Plan

> Execute inline with executing-plans and TDD; one final fresh read-only review.

**Goal:** Verify real restart persistence and implement the five requested native UI corrections.
**Architecture:** Existing shared Slint/controller and native status owner; an owned add-window coordinator mirrors presentation and forwards validated callbacks to existing commands. Native modal leases restore parent ownership/enabled state before window destruction.
**Tech Stack:** Rust 1.92, Slint 1.18.1, librqbit 9.0.1, existing objc2/windows-sys; locked Cargo, -j2, deployment14.
**Spec:** [explicit request and implementation choices](../specs/2026-10-27-native-polish-design.md).

## Global constraints and review focus

Keep primary dirty checkout and AppKit prototype untouched. No release/tags/installers. Preserve default-on durable tray intent, native owner lifetime, hidden CPU savings, safe commands and existing confirmation. No dependency version upgrade. Native parent must be restored on Cancel, Add, Quit and error. No callbacks for stale preview IDs; queued inputs remain serial. Child theme/language and folder-picker parent must agree with the displayed window. Preserve right-click multi-selection and keep ellipsis/global toolbar semantics.

## Task 1 — Menu presentation and sidebar

Files: src/ui/tray_status.rs, tests/tray_status.rs, src/platform/macos_tray.rs, ui/main.slint, examples/macos_lifecycle_probe.rs.
- [x] Verify ordinary-app OFF persistence through one actual restart, then restore ON and record bounded evidence.
- [x] RED `compact_summary_is_single_line_localized_and_keeps_aggregate_semantics`: exact EN/RU summary, no newline, correct mixed-state totals and active count, empty/overflow/other languages preserved.
- [x] Add `TrayStatus.summary: String`; native Mac owns one summary menu item, updates in place, keeps detailed tooltip. Set image template=true; use existing brand symbol at 24px in expanded header.
- [x] Focused tests + lifecycle/native template assertion and visual checks; commit.

## Task 2 — Restore row context menu

Files: ui/main.slint, ui/torrent-menu.slint, src/ui/controller.rs, src/ui/view_state.rs and matching tests.
- [x] RED context targeting: right-click selected preserves group; unselected replaces selection; stable ID after sorting; command targets/confirmation honor that selection and busy guard.
- [x] Connect actual right button to a positioned TorrentMenu, reusing row/group commands, Copy/Open folder only for single selection; keep ellipsis details and global Start all/Stop all unchanged. Provide keyboard context invocation.
- [x] GREEN focused real-controller tests and native right-click; commit.

## Task 3 — Owned modal add window

Create ui/add-window.slint, src/ui/add_window.rs, src/platform/modal.rs; modify ui/main.slint, controller.rs, ui/mod.rs, platform/mod.rs, Cargo.toml feature flags, lifecycle probe and focused tests.
- [x] RED coordinator tests with an injected native lease: one child, magnet→preview, snapshot binding, cancel/close, busy close guard, stale preview callbacks, serial queue/recreate, no window on inactive state, lease cleanup on error/shutdown.
- [x] Implement `AddWindowCoordinator` on UI thread. It owns optional AddWindow and modal lease; deferred/coalesced synchronization follows main add-state properties. Mirror models, errors, locale/theme; forward callbacks only for the displayed current preview ID. Expose weak child as picker parent. Remove inline add panes while preserving main content and existing callback API.
- [x] Implement native AppKit sheet/Win32 owner lease from validated window handles. Drop/end lease before hiding/destroying child; restore owner on all paths. Parent remains main-thread-owned; no nested modal loop, new timer or engine state duplication.
- [x] GREEN coordinator/real-controller tests; lifecycle exercise child modal attach/detach and hidden tray remains. Check full UI with owned local fixtures: file/magnet, long file list, cancel/Escape/close, folder picker, duplicate/queued input, theme/locale, main input blocked, parent usable after close.
- [x] Full fmt/Clippy/Rust/Python/catalog checks, fresh review with one fix pass, three-platform CI without packages. Build separate optimized test .app, paired evidence and commits. Preserve ordinary user's active transfer except the requested bounded restart.
