[Русский](monorepo-eta-validation-ru.md)
# Repository move and remaining-time display — 27 October 2026

The implementation evidence below was collected on the then-uncommitted tree `codex/monorepo-eta-20261027`, based on `58195bb`. No commit, push, main merge, remote CI, installer upload or release had been performed at that stage. The maintainer subsequently authorized commits, push and fresh three-platform CI with installer uploads disabled. The earlier branch's CI is not evidence for this migration.

## Preservation and layout

- The old primary checkout's 146 files, worktree/index patches and exact index were backed up under `_work/migration-backup/2026-10-27-before-monorepo`, with matching SHA-256 values. The original main reference remains intact. Original translations were integrated with newer plan and native validation notes in both languages.
- Desktop source/UI/tests/examples/build script/packaging moved into `apps/desktop`. The root is a Cargo workspace with exactly one package, selected by default. Website/mobile/shared-library directories contain explanatory placeholders, not claimed implementations.
- Shared assets/scripts/vendor remain at the root; fonts and OFL text moved into `assets/fonts`. Documents were grouped into brand/audits/architecture/validation with local links updated. The three original EN/RU document pairs retain their established paths.
- `output` and `tmp` moved intact into ignored `_work/brand/output` and `_work/brand/tmp`. All 908 file/symlink records match the move manifest. Nothing was overwritten. Generated packages and evidence stay in `_work`.
- Cargo.lock and dependency versions are unchanged. Engine, storage, persistence and schemas are byte-identical to the verified base. The separate AppKit prototype checkout is untouched.

## Time display

The user approved the longest current ETA among active unfinished torrents; paused entries are excluded, and any active unknown estimate makes the total unknown. The final tray display uses the existing Lucide clock, HH:MM (hours:minutes, rounded up), localized `less 1 min` below one minute and a dash when unknown. Mac places time first beside the native menu icon, then aggregate download/upload rates. Windows uses the clock on its third tray row. The tooltip/image description provides localized remaining-time text. No timer was added; same-rate changes within the same displayed minute do not wake the hidden UI.

## Verification

- Final fmt and strict all-target Clippy passed. Rust: 160 passing entries, no failures/ignored across 35 binaries. Python: 48 passed. Catalogs: 179 contextual messages across 14 languages.
- Native lifecycle passed: delegate/file/reopen callbacks, modal lease attach/detach and hidden event loop. Optimized ARM packaging and strict ad-hoc signature passed with exact-lock notices. The existing macOS 27 host-only strip workaround was used; normal executable optimization remains enabled. No Developer ID signing/notarization is claimed.
- A controlled loopback-only 4 MiB transfer completed with its window hidden. SHA-256: `4f093b0564a31d6d566aa522f8ff37f9dcadb46651b1beb52b398dc2becb01dc`. The user confirmed ETA in the native menu before the later clock/HH:MM presentation change.
- One fresh read-only review verified moved-file coverage and backup hashes, and found the Slint conditional-translation scan regression. A reproducing test failed; the suffix-based scanner fix passed. The final format has boundary/overflow/paused/unknown and minute-refresh tests.
- The final clock fixture initially waited for macOS Documents permission. The user granted that permission and unlocked the Mac; no security settings were bypassed. The native sheet added its controlled 8 MiB payload; right-click Stop/Start changed its actual state, and transfer continued with the window hidden. The user confirmed a real clock icon and time without ETA letters or seconds. Completion SHA-256: `ce19ea778aa3281f31d014ae0217fb7888ea6e29ecf24f5af77a11203f858abe`.
- The diagnostic app was shut down. The previous ordinary app exited gracefully and the new optimized bundle was launched. Both saved records appeared; the large active record was still Checking at the bounded observation, and the completed record was Seeding. This does not claim that the large transfer had already resumed or that an overnight test passed.

The final test package is `_work/exports/macos-monorepo-eta-20261027-clock/FlowTorrent.app`. The approved Windows/Intel CI must check the new committed revision; results are recorded by GitHub Actions. Physical Windows acceptance is separate. Later [checking-progress](checking-progress-validation.md) and [double-click](slint-double-click-validation.md) changes have their own evidence.
