[Русская версия](checking-progress-validation-ru.md)

# Checking progress — 2026-10-27

The restart screenshot of a 57 GB torrent showed Checking and an empty 0% bar. The pinned librqbit 9.0.1 already exposes initialization work through `TorrentStats.progress_bytes` and its scan total through `total_bytes`. The adapter deliberately keeps verified downloaded bytes at zero until initialization finishes; that safeguard remains.

The transient `TorrentSnapshot.checking` carries the separate processed/total scan counters only while initializing. Both paused-first restore and active initialization are supported. The UI shows the localized state plus a whole-number percentage, such as Checking 37% / Проверка 37%, and uses scan progress for the amber bar. The numeric bar label retains its existing one-decimal precision. After checking, both progress and state return to the download view. The scan denominator covers the engine's full scan, even when the selected download is smaller. Missing/zero totals show no invented percentage; counts are clamped and calculated without integer overflow.

There is no additional hashing, payload reading, polling, persistence or new dependency. The existing 500 ms snapshot publication carries the counters. Scan progress cannot count as verified downloaded bytes, affect download completion or cause a completion notification. Storage safety, manifest ownership and paused-first restore are unchanged. No AppKit prototype files were changed.

## Automated evidence on this Mac

- A new UI model regression failed before the display change: Checking was returned instead of Checking 37%; it passed after the change.
- Tests cover both languages, differing selected/scan totals, overflow and clamping, unknown totals, returning to download progress, initialization with either pause flag, ignoring counters outside initialization, and no completion notification at 100% scan progress.
- Pinned Rust 1.92.0, locked dependencies, `-j 2`, `MACOSX_DEPLOYMENT_TARGET=14.0`: formatting, strict Clippy and all 164 Rust tests across 35 test binaries passed.
- The macOS lifecycle probe, 48 packaging/tooling tests and 179 contextual messages across 14 languages passed.
- The optimized ARM Mac test bundle at `_work/exports/macos-checking-progress-20261027/FlowTorrent.app` passed package/notices validation and strict ad-hoc signature verification. It is not Developer ID signed or notarized and was not launched automatically.

Logs and source hashes are under `_work/validation/checking-progress-20261027/`. After receiving the new test build, the maintainer reported all checks working on this Mac. This is maintainer-reported acceptance of the display, not a captured trace of every verification phase. The agent did not stop, restart or edit the already-running user application, its download or its saved session. Windows acceptance remains separate. At the time of the implementation checks no commit, push or release had been performed; the maintainer subsequently authorized commits, push and CI with installer uploads disabled.
