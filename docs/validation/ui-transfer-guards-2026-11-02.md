[Русская версия](ui-transfer-guards-2026-11-02-ru.md)

# Transfer callback guards — 2 November 2026

Status: fixed and fully verified locally on Mac ARM; the identified callback debt is closed.

This closes the direct-callback debt from the [priority-menu review](priority-menus-2026-11-02.md), on `experiments/file-priorities`, base `142f2bb75a4c4c8769f15a44d0f4fface33406b0` plus preserved uncommitted work. No commit/push/merge/publication.

File selection, file priority and torrent priority now use the same transfer-edit gate. It rejects callbacks while the UI is not ready, busy, stopping, awaiting a command, showing Settings, add preview, magnet input/discovery, removal confirmation or About. File commands also reject metadata discovery in the authoritative snapshot before dialog flags catch up. Existing target/file ID checks and checking/error/hidden-state validation remain. Rejected file callbacks still restore canonical widget values, including when an identical operation is already pending.

The shared application command queue is unchanged: dialog confirmation, cancellation and settings saving must keep working within their own flows. No engine, manifest/schema, Slint layout, dependency, platform or AppKit-prototype code changed in this follow-up.

## Evidence

Tests call the real handlers installed by `bind_callbacks` on a headless Slint window, and inspect the real bounded mpsc receiver. They do not replace the handlers or mock engine behavior. Each closed gate has a positive control proving the same callback resumes and dispatches exactly one command with the actual torrent/FileId afterward. Blocked callbacks preserve pending state, lifecycle flags, existing errors and the About dialog.

Four new tests failed before the fix: commands escaped native chooser busy state with no pending operation, About, not-ready and metadata discovery. All four pass after the fix. Coverage includes all three edits; preview, magnet input/discovery, Settings, removal confirmation, stale busy/pending combinations, hidden snapshots and closing/reopening eligibility. Existing rendered widget rollback and stale-ID tests remain part of the full suite.

Final result: **199 passed, 0 failed, 0 ignored**, 44 harnesses. Four new tests cover 36 blocked calls and 36 successful dispatches after clearing gates. Fmt, strict all-target Clippy and Mac lifecycle passed. Standard release repeated the known local `zerofrom_derive` failure (E0463/E0432); release with `--config 'profile.release.build-override.strip="none"'` passed in 2m 41s. Cargo profiles were not changed.

Rust 1.92.0, locked dependencies, jobs 2, Mac deployment target 14.0. Raw logs, before/after source hashes and the small incremental controller diff are under ignored `_work/validation/ui-transfer-guards-20261102/`.

This is automated callback-boundary evidence, not a new physical native-dialog or Windows/Intel CI check. Running user/test applications were not restarted. Main, AppKit and other experiments were left untouched.

All 365 frozen inputs matched after verification. The only changed pre-existing source file is `apps/desktop/src/ui/controller.rs`; `controller_guard_tests.rs` is a new test module. Local release executable SHA256: `f47f11db65de4d7779d6932c82c7b49da4a6295ee6a0046015232871c440e421`.
