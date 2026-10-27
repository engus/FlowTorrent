[Русская версия](file-audit-2026-10-27-ru.md)

# File hygiene audit — 2026-10-27

## Scope and method

Inventoried the FlowTorrent tree, including local `target/` and `_work/` sizes, excluding Git internals. Examined the active source, assets, packaging scripts, workflows and documentation for resource references, followed the Slint import graph from the application and probe entry points, and compared active project files by SHA-256. Generated caches, packaged applications and recovery backups were inventoried without treating them as unused source. This is a file hygiene audit, not proof that every function or historical document is still needed.

The existing dirty working tree, engine, UI source, Cargo manifests and lockfile were preserved. The only configuration change for this audit is adding `/__todelete/` to `.gitignore`.

## Proposed deletions, quarantined for review

| Original path | Files | Reason |
| --- | ---: | --- |
| `assets/flow.svg` | 1 | Obsolete green brand symbol; no active resource references |
| `assets/magnet.svg` | 1 | Superseded by the Lucide magnet icon; no active resource references |
| `assets/actions/*.svg` | 13 | Former custom icons replaced by the Lucide set; no active resource references |
| `.DS_Store` in root, apps, apps/desktop, assets, docs, docs/superpowers | 6 | Finder metadata |

The 13 action icons are add, all, check, close, down, folder, more, retry, search, settings, start, stop and trash. Each legacy SVG matched its tracked version before moving; none contained uncommitted edits.

Moved 21 files (77,352 bytes) into ignored `__todelete/`, preserving their relative paths. The SVGs total 3,600 bytes. Every moved file was verified against its pre-move SHA-256. The quarantine contains English/Russian instructions and a per-file `manifest.json`. No permanent deletion, Git index modification, commit or release was performed. Quarantine does not free disk space.

## Retained deliberately

- `assets/flow.ico` is used by Windows packaging and the installer. Its old name does not make it obsolete.
- `assets/brand/FlowTorrent.icns` is a macOS packaging input. The current symbols and status icon remain in use.
- `assets/brand/logo-graphite.svg`, `logo-light.svg` and `symbol-light.svg` have no direct runtime references but are official brand variants suitable for the website and mobile app. They are real design work, not deletion candidates without a separate decision.
- All translation catalogs remain: they are loaded/generated through language tables rather than individual literal filenames.
- The complete curated Lucide set, `source.json`, README and license remain as a coherent asset package with provenance.
- No Slint files were unreachable from the application/probe import graph. Source modules, examples, tests and manual workflows were not removed on the basis of a missing text reference.
- The only exact duplicate active files found were Apache/MIT license copies in the two patched vendor crates. Both crates keep their own required license files.
- Specifications, audits, dated validation evidence, plans and translated documentation remain. Historical material is not obsolete merely because it describes an earlier state.

## Large local directories requiring a separate decision

| Directory | Logical file bytes at inventory | Decision |
| --- | ---: | --- |
| `target/` | 8,858,253,309 (~8.86 GB) | Keep: contains build output alongside test bundles/fixtures and earlier validation material; do not delete wholesale |
| `_work/` | 317,675,649 (~318 MB) | Keep: brand work, exports, validation evidence and the pre-migration recovery backup |

`_work/exports/` also contains the app launched for current testing. The backup at `_work/migration-backup/2026-10-27-before-monorepo/` protects prior local work and must remain until the maintainer explicitly approves its removal. Actual allocated space can differ from logical byte totals. No downloads or external AppKit worktrees were touched.

The machine-readable inventory, pre-audit source hashes, quarantine manifest and `verification.json` are saved locally under `_work/validation/file-audit-20261027/`.

## Verification

- All 21 quarantined files match their original SHA-256; all other 395 pre-existing project files match the pre-audit hashes, excluding the intentional `.gitignore` change.
- All 34 literal Slint/Rust embedded-resource paths checked still exist; the Windows packaging icon remains present.
- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/tests -v`: 48 tests passed.
- `python3 scripts/generate_translations.py --check`: 179 contextual messages, 14 languages verified.
- `cargo check --locked --all-targets -j 2`, Rust 1.92.0, `MACOSX_DEPLOYMENT_TARGET=14.0`: passed on this Mac.
- `git check-ignore __todelete/manifest.json`: the quarantine is ignored. The Git index remains unchanged.

This cleanup did not rerun the full Rust test suite, release packaging, Windows CI or physical UI tests. The existing test app was left running.
