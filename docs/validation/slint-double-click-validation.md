[Русская версия](slint-double-click-validation-ru.md)

# Slint torrent double-click — 2026-10-27

Double-clicking a torrent row invokes the same `details-requested(id)` callback as its three-dot button. The row and its separately interactive progress area use Slint's built-in double-click recognition. Single-click selection, progress/ETA clicks and right-click context handling remain in place. The new double-click route respects the busy guard used by the three-dot button. There are no engine, dependency, platform-specific or AppKit prototype changes.

The regression test dispatches real pointer press/release events into the rendered Slint window. It checks single-click selection without a drawer request, double-click requests for the correct torrent in the name and progress areas, suppression while busy, and right-click context targeting without a drawer request. Before the UI handlers were added, it failed with `double click did not request row details`; after the change it passed.

Verified on this Mac with pinned Rust 1.92.0, locked dependencies, two Cargo jobs and deployment target 14.0:

- Formatting and strict Clippy passed.
- All 161 Rust tests across 35 test binaries passed.
- All 48 packaging/tooling tests passed; 179 contextual messages across 14 languages verified.
- The macOS lifecycle probe passed.
- An optimized ARM Mac test app was built at `_work/exports/macos-double-click-20261027/FlowTorrent.app`; bundle validation, exact dependency notices and strict ad-hoc signature verification passed. It is not Developer ID signed or notarized and was not launched automatically.

Local logs and source hashes are in `_work/validation/double-click-20261027/`. After receiving the later checking-progress build, which includes double-click handling, the maintainer reported all checks working on this Mac. Individual physical click paths were not separately enumerated. Windows acceptance remains separate. The agent left the already-running application running; automated checks do not imply replacement of its executable.
