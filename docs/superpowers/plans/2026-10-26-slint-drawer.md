[Русский](2026-10-26-slint-drawer-ru.md)
# Slint bottom drawer implementation plan

> Execute inline using executing-plans; fresh final review after implementation.

**Goal:** Transfer the approved drawer layout and white light background into the shared native app.
**Architecture:** Existing controller/command/snapshot ownership. UI layout state is persisted with compatible preferences; file inspection is explicit and bounded. No engine or Trash redesign.
**Stack:** Pinned Rust 1.92, Slint 1.18.1, librqbit 9.0.1; static Lucide SVGs.
**Spec:** ../specs/2026-10-26-slint-drawer.md
**Constraints:** Existing codex/slint-release-readiness worktree; AppKit untouched; Cargo --locked/-j 2, macOS 14.0. No release/tags/installer uploads. RU/EN and licenses/badge retained.
**Review focus:** Failed/rapid settings changes and OS rollback; legacy exact limits; closing/filtering/hidden inspection; minimum window with long paths/many files; icon license inclusion in both packages.

## Task 1 — Automatic durable settings
Files: src/ui/controller.rs, ui/settings.slint, ui/rate-control.slint; existing headless controller test.
- [x] Extend headless test: changing preferences queues UpdateSettings without Save; completion keeps pane open; failure restores durable fields/theme; opening emits no write; legacy 20,000 B/s retained. Run focused lib test and observe failure.
- [x] Reuse bounded UpdateSettings and existing login rollback; hydrate fields from saved snapshots, keep settings open, auto-submit explicit preference/rate/folder changes. Remove editable custom limits and Save/Cancel UI. Run focused test, expect PASS; commit.

## Task 2 — Inspection/layout state and native drawer
Files: src/settings.rs, src/ui/{view_state,controller}.rs, ui/{main,details,theme}.slint, tests/{settings,view_state}.rs.
- [x] Add failing tests for explicit drawer inspection/close preserving selection and filter reconciliation; old settings layout defaults/round trip/bounds. Observe RED.
- [x] Add normalized UiLayout (180/false/320 defaults), last-applied layout tracking and layout-changed command. Add details-requested callback; ordinary selection stays closed; InspectDetails(None) on close/filter/hidden modes. Verify tests.
- [x] Build collapsible/resizable sidebar and bottom two-column drawer with pinned actions/search, virtual files, keyboard resizing/scrolling and minimum 800×640. White main light background, neutral dark theme. Native fixture rendering is the layout check; do not test literal styling source. Compile/smoke; commit.

## Task 3 — Unified icons and distribution notices
Files: assets/icons/lucide, ui/icons.slint and control imports; scripts/package_{macos,windows}.py and packaging tests.
- [x] Add packaging regression requiring Lucide license in assembled output; observe RED.
- [x] Copy required official pinned SVGs and full ISC/MIT license/provenance; replace every control/status glyph with the common icon global, keeping app/tray branding. White primary text. Include license in both packages; portable tests PASS; commit.

## Task 4 — Verification and handoff
- [x] Run fmt, locked all-target strict Clippy/tests, Python packaging tests, release ARM and Mac lifecycle probe. Test actual isolated engine/UI: details selection/open/close, pause/resume, files/search, auto settings/failure, white/dark RU/EN and compact window.
- [x] Fresh read-only final review; address important findings with reproducing tests. Package labeled local .app without replacing the installed app. Push authorized development branch for three-platform core CI, wait for results; uploads remain disabled. Record exact SHA/results and remaining physical gates in paired docs; commit docs only after verification.

Verified source `b00b27d`: [validation and remaining physical gates](../../validation/slint-drawer-validation.md).
