[Русский](2026-10-26-tray-preference-ru.md)
# Tray preference and background visibility Implementation Plan

> Use executing-plans inline, test-driven-development and systematic-debugging. A final read-only review covers the new change.

**Goal:** Transfer approved v45 menu-bar toggle into Slint and fix unintended icon disappearance after hiding the window.
**Architecture:** A compatible default-on UI preference travels through the existing durable settings transaction. Tray lifetime is owned independently of the main window. Explicit disable and exit remove the icon; hiding/reopening the window must preserve it. Investigate the native regression before choosing its fix.
**Tech Stack:** Rust 1.92.0, Slint 1.18.1, existing platform APIs and embedded fourteen-language catalogs.
**Spec:** User approved v45 mockup and requested native transfer/fix on 26 October 2026. Default enabled; automatic persistence; Mac menu-bar label and Dock hint; Windows tray label and reopen guidance. Existing path/session/notification invariants remain binding.

## Constraints and review focus

Reuse current isolated checkout. Preserve unrelated in-progress diagnostics until disposition; primary dirty main and AppKit prototype remain untouched. No release/tags/installer uploads. Locked Cargo, -j2, Mac deployment14. Old settings must load enabled; false must roundtrip; damaged settings must not be overwritten. Apply tray preference only from durable snapshots; save failure restores control and focus. Main-window visibility must not own tray lifetime. No background polling workaround. Keep one icon, current locale/rates, and terminal exit behavior across toggles. All new strings use embedded catalogs.

## Tasks

- [ ] RED settings migration/default/roundtrip and real UI save/rollback regressions; implement persistence, localized settings control and hidden refresh preference fingerprint.
- [ ] Reproduce/isolate native disappearance; implement and verify one root-cause fix, including disabled startup and off/on while hidden. Preserve Windows behavior and AppKit isolation.
- [ ] Full fmt/Clippy/Rust/Python/catalog checks, native lifecycle and physical owned-fixture checks; clean temporary diagnostics. Record actual results, fresh review, small commits. Keep source merge/publication gated on unresolved defects.
