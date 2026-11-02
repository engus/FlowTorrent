[Русская версия](2026-11-02-unslop-polish-ru.md)

# Unslop polish implementation plan

> Execute inline using executing-plans. The maintainer already approved the three visual proposals and requested implementation. Steps track verified work only.

**Goal:** transfer approved main/settings/add polish without changing behavior.
**Architecture:** presentation-only Slint changes; existing callbacks/models remain authoritative. Keep the latest priority/guard implementation at base `10d647b`.
**Tech stack:** Rust 1.92.0, Slint 1.18.1, locked dependencies, jobs 2, Mac target 14.0.
**Spec:** [approved scope](../../architecture/unslop-polish.md); existing product specification and subsequently approved features remain binding.

## Global constraints

No engine/platform/persistence/manifest/schema/dependency changes. No ordinary user session, main or other worktree changes. Existing settings, priorities, sources, selection, search, ETA/ratio, native window ownership and Slint attribution remain. Preserve all languages. No publication, installer upload or merge.

## Review focus

Minimum 800×640 main window and 480×360 add window; long RU/EN labels/paths; dark theme contrast; keyboard/disabled/busy/error paths; menu positioning and centered priority glyphs. Use existing real rendered/controller/ownership tests plus native inspection. Do not add source-string or constant/screenshot-change detectors for reversible styling.

### Task 1: Shared theme and main window

- [x] Baseline: run `cargo test --locked --test priority_menus_ui -j 2` on unchanged base; expect 2 passing tests.
- [x] Update `ui/theme.slint`, `main.slint`, `details.slint`, `add-control.slint`, `priority.slint`: palette, sentence case, alignment, 16 px icons, 600 emphasis, consistent spacing. Keep model/callback/row-height contracts. Make the existing glyph-alignment test recognize saturated state ink independently of its intentional hue.
- [x] Run existing rendered priority tests and controller tests; expect preserved behavior and aligned glyphs.

### Task 2: Settings and add presentation

- [x] Update `ui/settings.slint`, `setting-switch.slint`, `theme-selector.slint`, `rate-control.slint`, `add-torrent.slint`, `add-magnet.slint`: readable typography, grouped spacing, explicit final-location/selected-size hierarchy and shared button styling. Keep every existing guard, binding, callback and focus route.
- [x] Update `assets/locales/*.json` and regenerate `src/ui/translations_generated.rs`; expect `python3 scripts/generate_translations.py --check` success.
- [x] Run existing add-window/settings/controller tests and full checks. No new behavioral feature is introduced; existing tests are the regression gate.

### Task 3: Verification and delivery

- [x] Run fmt, strict all-target Clippy, all-target tests, Python checks, release and Mac lifecycle with pinned locked toolchain/jobs 2. Record any existing host-only release workaround separately.
- [x] Inspect a fresh isolated native harness at compact/wide sizes, RU/EN, light/dark, main/settings/add and keyboard controls; do not use the ordinary user's registry.
- [x] Whole-diff review, paired validation report and handoff. Preserve actual review/build limits and provide the reviewable native candidate.

Pre-flight: Task 2 consumes shared theme/buttons from Task 1. Existing models, callbacks and native coordinator are unchanged. Presentation-only styling uses existing behavior tests; source-string/constant tests would add no useful coverage.

Validation details and limits: [report](../../validation/unslop-polish-2026-11-02.md). Add was inspected at its normal 640x560 size; physical 480x360 resizing remains unverified.
