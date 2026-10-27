[Русский](2026-10-27-monorepo-eta-ru.md)
# Monorepo and ETA Implementation Plan

> Execute inline with executing-plans and TDD; one fresh read-only final review. User forbids commits without confirmation.

**Goal:** Complete the approved folder move and replace tray downloading count with aggregate active ETA.
**Architecture:** Root Cargo workspace with one unchanged desktop package; shared root build assets/scripts/vendor; typed event-driven tray presentation.
**Tech Stack:** Rust1.92, Slint1.18.1, librqbit9.0.1; locked Cargo, -j2, deployment14.
**Spec:** [Approved scope](../specs/2026-10-27-monorepo-eta.md).

## Global constraints
Preserve local work/backups, pinned versions, Cargo.lock, all storage/session safety, native tray ownership and isolated AppKit work. No commits/pushes/uploads/releases. Root cargo commands remain usable. Physical checks and CI evidence are distinct.

## Review focus
Workspace-root versus package-root paths; silent translation scanning of obsolete src; notices/package assets and Windows commands; paired document links and newer evidence; ETA unknown/paused/overflow behavior and hidden refresh.

- [x] Save original dirty primary files and index, verify hashes, branch from latest verified source without committing.
- [x] RED: active max ETA, paused exclusion, unknown active rows, completed rows, ceil/overflow and same-rate progress refresh in tests/tray_status.rs. RED: nested desktop source must be discovered by catalog scanner.
- [x] Move package to apps/desktop; virtual root workspace retains release profile/vendor patches. Move fonts/packaging; update Rust/Slint asset paths, Python scans/package inputs and matching tests. Create website/mobile/crates placeholders and folder docs. Relocate docs with internal-link rewrite and preserve original bilingual work/newer validation notes. Move brand folders with hash manifests; add ignored _work.
- [x] GREEN ETA display on both platforms, no timer and no engine change. Keep refresh disabled behavior. Validate metadata --locked, fmt, strict Clippy, all Rust/Python/catalog tests and native lifecycle. Inspect lockfile/version changes (none expected).
- [ ] Build a labeled local optimized Mac app with exact notices; native local-fixture transfer/modal/context and real tray ETA check where accessible. One fresh review, one fix pass with tests for Important/Critical findings. Record actual evidence and proposed commits; await maintainer approval before committing or CI push.
