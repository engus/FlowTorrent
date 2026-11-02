[Русская версия](2026-10-31-file-selection-ru.md)

# Stage 1: file selection

Spec: [approved experimental behavior](../../architecture/file-selection-experiment.md).
Worktree: `file-priorities/FlowTorrent`, branch `experiments/file-priorities`, base `142f2bb`.

1. Test-first manifest v2 / empty restore / notification guard, preserving v1 and future/corrupt data protection.
2. Test-first paused engine selection update with validated IDs; verify local multi-file download, re-enable and shared pieces.
3. Test-first serialized controller toggle with durable intent, error recovery, paused/active preservation, restart and rapid toggles; cover trash ownership.
4. Add single-torrent Slint checkboxes, real FileId callbacks, disabled controls during Checking/Error, and localized empty-selection state.
5. Run fmt, strict Clippy, all-target tests, release and lifecycle probe at -j2/locked. Fresh whole-branch review, fix important findings, record actual evidence and limits. No commits/push/merge/publication.

Review focus: failed saves and engine operations, interrupted transactions, no selected files, restored empty selection, old clients, stale UI commands, already-complete torrents, shared boundary pieces, pinned Windows handles, payload ownership in Trash, no false completion or notification.

## Verified outcome

Stages 1–4 complete. Stage 5: 176/176 test entries (0 ignored), fmt and strict Clippy, local release with documented build override, and Mac lifecycle passed. Fresh review P2 findings and rendered-checkbox enqueue rollback reproduced RED and passed GREEN. [Evidence](../../validation/file-selection-stage1-2026-10-31.md). Windows/Intel CI and physical checks remain open; stage 2 priorities are not implemented. Worktree and all logs preserved, no commits/push/merge.
