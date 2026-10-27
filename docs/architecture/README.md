[Русский](README-ru.md)
# Repository layout

The repository root is a Cargo workspace. The Windows/macOS Slint package, UI, integration fixtures and packaging inputs live in apps/desktop. Engine and app logic stay in that package until another app needs a shared crate.

- apps/website and apps/mobile reserve future app source locations; they do not select technologies or provide builds.
- assets contains shared build inputs; fonts and their OFL license live in assets/fonts. Preserve license text with redistributed resources.
- scripts and vendor remain shared at the root. Root Cargo commands and Cargo.lock remain the supported entry point.
- docs/brand holds the original brandbooks, docs/audits the audit record, docs/validation the dated evidence, and docs/superpowers the specs/plans. Historical reports retain their revisions and acceptance limits.
- _work is ignored and holds valuable local design material as well as exports. Never delete its real work without the maintainer's approval. Original output and tmp were moved intact to _work/brand/output and _work/brand/tmp, with file hashes recorded in _work/validation.
- _work/migration-backup retains the old primary checkout's local files, patches and exact index for recovery. Do not restore that old index over the new layout.
- target remains the ignored Cargo output directory. No dependency versions or application/session identifiers changed as part of the move.

The separate AppKit evaluation checkout is untouched. It has not become the production UI. No commits or remote CI are claimed for this uncommitted migration.
