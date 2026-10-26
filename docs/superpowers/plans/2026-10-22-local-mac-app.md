# Local Mac App Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan. Steps use checkbox syntax.

**Goal:** Deliver a locally runnable FlowTorrent.app and a repeatable packaging command.

**Architecture:** Package the existing release executable with native Mac tools. Keep the shared application and Windows configuration intact.

**Tech Stack:** Existing Rust 1.92/Slint/librqbit, Python 3.9 stdlib, sips/iconutil/codesign.

**Spec:** `docs/superpowers/specs/2026-10-22-local-mac-app-design.md`; existing 1.0 specification and task 10.

## Global Constraints

- macOS 14.0+, native ARM or explicit Intel target; ID `com.engus.flowtorrent`.
- Version from Cargo; `.torrent` Viewer/Alternate registration, no forced default.
- Local ad-hoc signing only; no publishing, credentials or user session cleanup.
- Preserve the original checkout's in-progress documentation edits.
- No shared Rust behavior or Windows profile changes for packaging.

## Review Focus

- A path with spaces/Unicode must produce a usable bundle (assembly test).
- Existing unrelated/symlink destinations must survive rebuild attempts (publish tests).
- Replacement failure must preserve the previous bundle (rename fault test).
- Incorrect architecture/deployment metadata must be rejected (native bundle verification).
- Finder cold/warm open and close/reopen/Quit must use one instance (local UI checks).

## Task 1: Local packaging command and deliverable

**Files:** create `scripts/package-macos.sh`, `scripts/package_macos.py`,
`scripts/tests/test_package_macos.py`, `packaging/macos/Info.plist`,
`docs/local-macos.md`, `.github/workflows/package-macos.yml`; update README and CI.

**Interfaces:** `assemble_bundle(binary, icon, output, version)` writes a fresh
bundle; `publish_bundle(staged, destination, replace=False)` safely publishes it.
CLI builds a locked release, creates/verifies icon and signature, then prints
the bundle path. `--replace` only replaces FlowTorrent bundles; `--output`
selects a destination; `--target` chooses an existing supported Mac triple.

- [x] Write behavior tests for assembly and safe publishing; observe failures.
- [x] Implement bundle metadata, assembly/publishing, native-tools CLI and wrapper.
- [x] Run Python tests; build/sign/verify the real native bundle.
- [x] Test Finder launch, preview/cancel, background hide/reopen and Quit locally.
- [x] Retain Windows CI and add Python packaging tests; add manual Mac artifact workflow.
- [x] Document one-command usage and actual verification; run Rust checks/tests.
- [x] Obtain one independent review, resolve important findings, deliver app path.

No commit, merge, push or public release is required for this local deliverable.
