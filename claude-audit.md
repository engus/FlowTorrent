# FlowTorrent audit

Audit date: 22 October 2026. Audited commit: `47106f2` on `main`. Author: Claude (Opus 5.5) in Claude Code.

## Scope and method

- **Read in full:** everything under `src/`, `main.rs`, `Cargo.toml`, the toolchain and Cargo config, `build.rs`, both CI workflows, the README and the handoff.
- **Skimmed:** the Slint UI files, the test files, `scripts/validate_resources.py`, the resource example and the vendored `librqbit-lsd` patch. The patch was not compared against upstream.
- **Not run locally:** Rust isn't installed on the audit machine, so nothing was built, linted or tested there. This is a static review. For build status it relies on CI: the `Check` run for `47106f2` passed on Windows x64, Mac ARM and Mac Intel.

## Verdict

No critical or security-blocking issues. The four medium findings are all about error handling and edge cases, not the core torrent logic.

## Strengths

- **Defence against hostile torrents is strong.**
  - Every file opens relative to a pinned folder handle (cap-std) without following links.
  - It rejects `..`, Windows device names, case-insensitive duplicate names and hard links on restore.
  - The storage layer refuses to delete files.
  - Bencode nesting depth and metadata size (16 MiB) are capped.
- **Single-instance messaging is locked down.** On Windows the named pipe grants access only to its owner (DACL `D:P(A;;GA;;;OW)`). On Unix the socket lives in a `0700` folder that is checked for owner and permissions. Messages are versioned and limited in size and path count.
- **Saving state is crash-safe.** Each save writes a temp file, syncs it, renames it over the old one and keeps a `.bak`. Damaged files and future schema versions are never silently overwritten.
- **Notification text is safe.** Windows toast text goes in as plain text nodes, not markup, and control characters are stripped.
- **`unsafe` code is minimal** (18 blocks), and each block has a SAFETY comment.
- **CI follows good practice:** actions pinned by SHA, read-only permissions, a pinned toolchain, `--locked` builds and Clippy with `-D warnings`.

## Medium

### 1. A slow or failed shutdown shows "FlowTorrent could not start."

If shutdown takes over 20 seconds (`src/main.rs:129`) or the event loop fails, `run()` returns an error at `src/main.rs:135-136`. `main()` then shows the startup-failure dialog at quit time.

**Fix:** give shutdown errors their own path, or at least only log them.

### 2. One damaged settings file locks the whole app.

- If `settings.json` or `session.json` fails to load, the app goes read-only (`src/app/controller.rs:111-114`).
- The user sees only a generic message, with no file path or next step (`src/ui/i18n.rs:46`).
- Saving settings re-reads the damaged file first and refuses to continue (`src/settings.rs:76`), so the user can't fix it from inside the app.

**Fix:** for four settings, use defaults in memory, keep the bad file untouched, and show its path. The spec's rule against silently overwriting damaged state would still hold.

### 3. A failed or cancelled add leaves empty files that then block that folder.

- When you confirm, `claim_files` creates every non-padding file, unselected ones included, before the session is saved (`src/app/controller.rs:297`, `src/app/targets.rs:316`).
- If a later step fails and you cancel or pick another folder, those files stay on disk.
- Re-adding to the same folder then hits a FileConflict caused by the app's own files.

**Fix:** track files the app created during an add that never finished, and remove them on cancel. Alternatively, allow re-claiming files that are still empty and that this process created.

### 4. The full preview is copied every 500 ms.

`AppSnapshot.preview` holds the raw metadata, up to 16 MiB, plus the file list. It is copied on every 500 ms tick (`src/app/controller.rs:496`) and again in the UI pump (`src/ui/controller.rs:175`). The handoff already notes this.

**Fix:** wrap the preview or its metadata in an `Arc`. It's a small, low-risk change.

## Low

5. **Removing a torrent leaves its saved copy.** `metadata/<infohash>.torrent` is never deleted (`src/app/controller.rs:229-248`).
6. **Saved state and the engine can drift apart.**
   - **Remove:** the session is saved without the torrent before the engine drops it. If the engine call fails, the paused torrent stays loaded but invisible until restart.
   - **Pause:** the engine pauses first. If the save then fails, the torrent comes back active after a restart.
7. **Startup errors are never logged.** Failures before `diagnostics::initialize` (`src/main.rs:71`) have nowhere to go. Also, launching with more than 64 files shows the generic "could not start" dialog (`src/main.rs:77`).
8. **CI has no dependency or license scanning.** There are 766 crates in `Cargo.lock`, and the release checklist requires exact license collection. Adding `cargo-deny` (advisories, licenses, sources) and Dependabot would cover both.
9. **Some code is hard to read.**
   - Compressed one-line blocks inside `tokio::select!` and closures that rustfmt skips: `src/app/controller.rs:119-136`, `src/ui/controller.rs:165-190` and `227-241`.
   - A 108 px row height is hardcoded for scrolling (`src/ui/controller.rs:473`), so it will break silently if the Slint layout changes.
10. **The single-instance setup has two minor weak spots.**
    - The `/tmp/flowtorrent-<uid>-<hash>` name is predictable, so another local user could create it first and stop the app from starting ("Unsafe local IPC directory"). That is a local denial of service only.
    - The name relies on `DefaultHasher` staying stable across Rust versions, which the handoff already flags.

## Suggested order

1. Findings 1 and 2: both are visible to users and the fixes are small.
2. Findings 3 and 4.
3. Add `cargo-deny` to CI (finding 8) before stage 10 packaging, since it also covers the license-collection gate.
4. The remaining low-severity items when convenient.
