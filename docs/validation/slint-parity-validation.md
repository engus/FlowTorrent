[Русский](slint-parity-validation-ru.md)
# Slint layout parity and Windows CI — 26 October 2026

Tested code: `53a41e8ba80d2c6802ccd5006d64c2a8976c358f`, branch `codex/slint-release-readiness`. The approved v42 prototype is the visual reference. AppKit remains isolated. The installed user app and its paused NAS payload were not changed. No GitHub Release, tag or installer upload was made.

## Corrected differences

- Restored sidebar branding, flat navigation and the category heading at the left of the toolbar. The wordmark switches to the symbol in narrow windows; search, global actions and Add stay at the right.
- Replaced 112 px cards with 70 px aligned table rows and a 43 px header. Narrow layouts collapse columns and constrain detail width. Selection has a quiet surface and leading accent. Completed paused rows retain the completed color; errors take precedence.
- Matched brand graphite/cyan colors, dark surfaces, compact typography and joined Add/magnet corners. Native chrome and native form controls remain.
- Compacted facts and folder groups. File heading/search and actions remain pinned while long facts/errors and the virtualized file list scroll independently. Metadata supports visible focus and arrow/Page/Home/End keys. Long names/paths are bounded with full-value tooltips.
- Progress is keyboard-focusable across its hit area. Completed rows open upload details while retaining upload speed; incomplete rows toggle ETA. Size/downloaded use checked bytes. Unknown restore metrics show “—”, unknown ETA and unavailable files, without implying zero bytes or completion. Missing-folder help asks to check the original drive/access and retry; relocation is not implemented.
- Footer Trash offers two removal choices; row ellipsis retains full actions. Global Start all/Stop all affect the complete registry. Ordinary row clicks show details without checkboxes; search/modifier selection is retained.
- Settings have a bounded form, sections, Save/Cancel and keyboard paging. Theme preview cancellation and saving work. Details retain labeled Start/Stop and compact folder/Trash icons.

## Windows failures and fixes

| Run | Correction |
| --- | --- |
| [40269171261](https://github.com/engus/FlowTorrent/actions/runs/40269171261), `e79e23e` | Both Macs passed. Windows login rollback test waited for a recovery marker absent when Windows reports a file-as-ancestor as NotFound. `ff1831b` waits for actual UpdateSettings completion, asserts apply/rollback and restoration of the previous OS value. Production login behavior is unchanged. |
| [40272076439](https://github.com/engus/FlowTorrent/actions/runs/40272076439), `b6e33ad` | Windows cannot move the pinned staging directory. `836955e` hands its single validated payload root to native Trash, keeping the private wrapper pinned. |
| [40274031981](https://github.com/engus/FlowTorrent/actions/runs/40274031981), `836955e` | Single-file/recovery passed; multifile folders retained prepared file handles. `b833e01` consumes caller PreparedAdd ownership after durable staging, releasing those handles before handoff. |

Final Windows core checks passed these regressions. A Windows-only retained-owner test requires safe failure with a recovery journal, then successful retry after release. Engine cancellation may briefly retain another owner; shared handles are never forcibly cleared. SecureStorage deletion still fails.

The trashed receipt persists before manifest removal. Wrapper cleanup is empty-only, nonrecursive and best effort. On Windows cap-std closes the directory handle before empty removal by name: cleanup is not identity-atomic. Global sharing flags and path validation were not weakened.

## Full workflow

[Check 40276244577](https://github.com/engus/FlowTorrent/actions/runs/40276244577) was manually dispatched with `test_packages=true` for the exact code above. The reusable workflow receives `upload_artifacts=false`.

Final conclusion: **success**, all eight jobs successful.

| Target | Core checks | Test packages |
| --- | --- | --- |
| Windows x64 | 131 application test records plus 18 bencode tests; zero failures/ignored | Passed tests, Clippy, notices, unsigned installer and disposable install/reinstall/uninstall preservation check |
| Mac ARM | 133 application test records plus 18 bencode tests; zero failures/ignored | Passed tests, Clippy, notices, .app and verified DMG |
| Mac Intel | 133 application test records plus 18 bencode tests; zero failures/ignored | Passed tests, Clippy, notices, .app and verified DMG |

Installer upload steps are **skipped on all three targets**. Artifact inventory contains exactly one item, `dependency-policy-inventory` (JSON), with no installers or DMGs. Windows smoke confirms default-handler, state/payload hashes and unowned-file preservation. Raw evidence: `final-ci-run.json`, `final-ci-artifacts.json`, `windows-final-{core,package}.log`, `mac-arm-final-{core,package}.log` and `mac-intel-final-{core,package}.log` in the evidence directory below. A subsequent documentation-only commit does not change this tested code revision.

Core checks include formatting, packaging tests, Windows metadata, locked all-target check/Clippy/tests, bounded bencode tests and both Mac lifecycle probes. Package jobs generate exact-lockfile notices, build unsigned Windows/Mac packages and run disposable Windows install/reinstall/uninstall preservation checks. CI does not certify physical DPI, keyboard, tray, Recycle Bin interaction or login/logout.

## Local verification and rendered comparison

Rust 1.92.0, `--locked`, `-j 2`, macOS deployment 14.0: fmt, strict all-target Clippy, **133 Rust test records across 33 completed test targets**, **40 Python packaging tests**, ARM64 release and the Mac lifecycle probe passed. Trash tests: 9/9 locally, plus the Windows-only retained-owner test in CI. Checked bytes, unknown restore metrics, Trash handoff and ownership have reproduced red/green regressions. Read-only independent review left no actionable findings.

Logs remain in ignored `target/slint-parity-validation/`: `fmt-final.log`, `clippy-final.log`, `tests-final.log`, `packaging-tests-final.log`, `release-final.log`, `lifecycle-final.log`, `package-final.log` and red/green logs. No fresh CPU/RAM measurement is claimed. The lifecycle probe exercises production delegate integration, not the isolated AppKit prototype.

Reference: `.superpowers/brainstorm/71364-1792932748/content/slint-redesign-v42-audit-fixes.html` and `prototype-validated-preview.jpg`. Controlled 40-file data differ from browser demo values; comparison concerns structure, hierarchy, colors and behavior.

Native checks covered RU/light and EN/light wide, RU/dark compact 800×640 long names/paths, EN/dark responsive and missing-folder Error. Confirmed ordinary selection without checkboxes, search/no-results/Escape, file filtering, pinned actions/two removal options, row menu, metadata focus/Home/End, Tab into file search, Settings Page Up/Page Down, theme Cancel and Dark Save with focus returned to Add. Login was not enabled. Fixtures used separate registries and loopback sources; harnesses and temporary reference server were stopped.

Screenshots: `native-light-en-wide.png`, `native-light-ru-wide.png`, `native-dark-ru-compact-long.png`, `native-dark-ru-keyboard-end.png`, `native-dark-ru-compact-error.png`, `native-dark-en-responsive.png`. Mac scale is 2×; English responsive image is 1600×1734 physical pixels. Earlier Error image precedes the final unknown-progress dash, which has a model regression.

Real native Mac Trash at `836955e` moved the controlled 57-byte `FlowTorrent-owned-trash-parity-20261026.bin` through UI confirmation. Native API succeeded; source/registry disappeared, with no recovery journal/empty wrapper left. Evidence: `native-trash-fixture.json`, `native-trash-result.json`. macOS denied ~/.Trash reads, so restored bytes/inode were not compared. Protection was not changed and Trash was not emptied. Later ownership changes passed local/Windows tests.

## Candidate and remaining gates

`target/local-macos/slint-parity-20261026/FlowTorrent.app`: ARM64, macOS 14+, ad-hoc test signature, no Developer ID/notarization. Binary 30,422,896 bytes; bundle 36,149,897 bytes. Adjacent `build-evidence.json` records source `53a41e8` and binary SHA-256 `d6d0a69f7df14c5228221847883b216c3234c651305b14844091e71aef078b34`. Quit any existing FlowTorrent first to test this executable; the ordinary candidate uses the ordinary registry, unlike smoke wrappers.

Physical Windows DPI/keyboard/tray/Recycle Bin/login, physical Intel execution, supported-system clean installs, overnight workload and signing remain release gates. Upload percentage is session-scoped; relocation and lifetime statistics are outside this transfer. Existing release gates are not cleared by this report. AppKit was not modified and no release was published.
