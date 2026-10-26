# Changelog

## Unreleased — Slint 1.0 preparation

No public 1.0 release has been published. Local test artifacts retain Cargo's
development version, 0.1.0.

- Shared Rust/Slint client for Windows x64 and Mac ARM64/Intel.
- Add torrent file or v1/hybrid magnet metadata, file/destination preview,
  download, pause/resume, seeding and saved-session recovery.
- Safe no-clobber payload storage, durable intent, protected damaged state,
  settings recovery and actionable startup/runtime/shutdown errors.
- Locked dependency checks and memory-duplication fixes.
- Local Mac `.app`/DMG packaging, target-specific notices, checksums and
  artifact manifests; exact macOS 14.0 deployment-floor verification.
- Per-user Windows test-installer implementation with native version/icon
  resources, optional default-app candidacy and data-preserving uninstall design.
- Manual test-package workflow; installer uploads require an explicit input.

Remaining acceptance checks are tracked in [the release checklist](docs/release-checklist.md).
Native AppKit prototypes are separate experiments, not the Slint 1.0 frontend.
