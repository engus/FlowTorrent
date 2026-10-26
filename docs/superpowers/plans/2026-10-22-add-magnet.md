# Add torrent and magnet implementation plan

Use superpowers:executing-plans inline. Spec:
`docs/superpowers/specs/2026-10-22-add-magnet-design.md`.

## Task 1: Add control and magnet preview flow

- [x] Add failing tests for link validation, loopback metadata-only preview,
  controller confirmation/queue ordering and cancel/Quit during lookup.
- [x] Implement bounded magnet parsing and owned async inspection, plus a
  cancellable controller job that feeds the normal preview queue.
- [x] Add reusable split control, paste/progress pane, EN/RU copy and callbacks.
- [x] Run Rust/Python checks, rebuild the Mac app and test both entry paths.
- [x] Perform one independent review; fix important findings; document results.

Review focus: untrusted magnet query ranges must not allocate unboundedly;
metadata lookup must never create payloads or persist a torrent before confirm;
Cancel/Quit must work without waiting for peers; queued local files must survive
magnet success/failure/cancel; disabled/focus/accessibility behavior must work.

Keep the existing attached worktree and leave changes uncommitted. Rebuild the
existing local app only after quitting it, preserving user state.
