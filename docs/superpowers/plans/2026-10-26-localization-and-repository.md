[Русский](2026-10-26-localization-and-repository-ru.md)
# Localization and repository preparation Implementation Plan

> **For agentic workers:** Use executing-plans for integration and dispatching-parallel-agents only for disjoint translation catalogs after their shared source catalog is fixed. One fresh final reviewer; no per-language reviewers. Checkboxes record verified results only.

**Goal:** Prepare About, live tray statistics, fourteen-language UI and repository documentation, then merge the verified shared Slint implementation into main without publishing a release.

**Architecture:** Preserve Rust/librqbit/Slint and the snapshot command boundary. Keep RU/EN call sites as translation context; a generated static catalog maps (English, Russian) pairs to twelve new languages. Slint uses an exported locale-aware translation global; Rust uses the same catalog. Catalog data is embedded, with no runtime network or per-string JSON parsing. Existing settings migrate by keeping System/Ru/En serialized values. Snapshot-derived tray rates/counts update while hidden without a new timer.

**Tech Stack:** Rust 1.92.0, Slint 1.18.1, librqbit 9.0.1, existing serde_json and Python standard library. No dependency upgrades or new runtime dependencies.

**Spec:** User request of 26 October and confirmed choices: es, pt, fr, de, it, pl, tr, ja, ko, zh-CN, kk, uk in addition to en/ru; a disabled Support the author button until a Ko-fi URL is supplied. https://golota.me is the author destination. Existing safety specs and later explicitly approved features remain binding.

## Global Constraints

- Work in existing codex/slint-release-readiness checkout; leave AppKit and the dirty primary main worktree unchanged.
- Preserve manifest/path/Trash/notifications invariants and no public swarm in tests. Locked builds, local -j 2, macOS deployment14.
- No public release, tags or installer uploads; only source merge is authorized.
- Preserve exact existing RU/EN settings values and damaged/future settings behavior; unavailable translation falls back to English.
- Brandbook originals stay unchanged; publish copies with provenance and licenses under docs/brandbook.
- Main merge uses remote PR/API after exact source CI/review; do not reset or overwrite the primary checkout's uncommitted documentation.

## Review Focus

- Duplicate English words with different RU contexts (Download action versus rate noun) must not share the wrong translation; test contextual lookup.
- New language persistence/system tags, unsupported Chinese variants, unknown tags and old settings must retain intended behavior; test round trips and detection.
- Switching locale must update every mounted pane, About/native Help/tray/notifications and revert after failed save; exercise controller binding and native long/CJK/kk/uk views.
- Disabled support must never launch a browser or receive trapped focus; test fixed URL allowlist and About lifecycle.
- Hidden tray counts use actual Downloading states and saturating registry totals, excluding paused/waiting/seeding; test aggregate semantics and zero transitions, no extra polling.

### Task 1: About and tray status
**Files:** src/ui/about.rs, ui/about.slint, ui/tray.slint, src/platform/mod.rs, src/ui/controller.rs; new src/ui/tray_status.rs, tests/tray_status.rs.
**Interfaces:** About link0 project, link1 unavailable support, link2 https://golota.me; support_available=false. TrayStatus::from_rows(&[TorrentSnapshot],Locale) produces localized rates/count and tooltip; PlatformShell::set_status accepts it and changes only differing properties.
- [x] RED: About allowlist rejects support and accepts author; browser failure identifies author URL. Tray aggregation covers mixed states, empty and overflow.
- [x] GREEN: Implement copy, disabled focus-aware button, author link, read-only menu status rows and tooltip from existing snapshots, independent of visible window.
- [x] Run focused tests and commit.

### Task 2: Shared localization foundation and catalogs
**Files:** assets/locales/messages.json and twelve language JSON files; scripts/generate_translations.py; src/ui/translations_generated.rs, src/ui/i18n.rs; ui/translations.slint and localized UI imports; src/settings.rs; src/ui/controller.rs; src/platform/mod.rs.
**Interfaces:** Locale::ALL, Locale::from_system_tag, Locale::index/from_index, Locale::text(ru,en); LanguagePreference keeps System/Ru/En plus twelve new variants. Translation global lookup(locale,ru,en) reacts to locale property. Generated static arrays keyed by contextual pair; English fallback without allocations. Settings list includes System plus fourteen autonyms.
- [x] RED: locale detection/serde compatibility/context lookup and generation key/placeholder completeness; new locale changes affect a visible UI label and About.
- [x] Prepare source catalog including About/tray additions; delegate only separate catalogs, four languages per translator.
- [x] GREEN: Integrate shared lookup, replace every RU/EN UI ternary, migrate settings choice handling and all native strings. Generate/check deterministic arrays; verify no untranslated user-facing RU/EN conditional remains.
- [x] Focused Rust/Python tests and commit complete catalogs.

### Task 3: Repository documents and brandbook
**Files:** README.md, README-ru.md, docs/README.md/-ru, docs/user-guide.md/-ru, docs/repository-about.md/-ru, docs/localization.md/-ru, docs/brandbook/* and paired handoff/evidence.
- [x] Describe actual current features, supported OS, installation/app/DMG versus Windows installer, testing status, build commands, privacy, licensing, support placeholder and author link.
- [x] Copy original EN/RU brandbook PDFs from primary output/brandbook/flowtorrent-brand-kit with SHA/provenance; link canonical assets/icons/fonts/licenses without moving old evidence documents.
- [x] Verify document links and copies; prepare GitHub About description/topics and commit.

### Task 4: Native QA, full verification and source merge
- [ ] Native owned fixtures: About disabled support/author/keyboard focus, tray menu visible+hidden and zero/active status, long Latin and CJK/kk/uk labels, settings failure fallback. Do not change user's login/notification permissions or NAS torrent.
- [ ] fmt, Clippy, all-target tests, Python/generator checks, ARM release and lifecycle; package separate unsigned test.app with notices.
- [ ] One read-only fresh review against this plan and complete branch intent; fix Important/Critical with a reproducing test and green suite.
- [ ] Push feature branch, run three-platform core CI with test-packages skipped; create/attach PR, update repository About via supported API/CLI, merge to main after all checks pass. No Release/tags/installers.
- [ ] Verify main contains source and final CI, save paired evidence/handoff and report remaining physical/native-speaker acceptance limits. Preserve dirty primary checkout.
