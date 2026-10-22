# Claude audit implementation plan

Execute inline with superpowers:executing-plans. Spec:
`docs/superpowers/specs/2026-10-22-claude-audit-fixes-design.md`.
Baseline: local Mac and magnet work committed before these fixes.

## Task 1: Lifecycle diagnostics (audit 1, 7)
- [x] Reproduce shutdown timeout classification and early logging failures.
- [x] Separate lifecycle error context, log early failures, show useful argument errors.
- [x] Test and commit the focused fix.

## Task 2: Settings recovery (audit 2)
- [x] Reproduce corrupt-settings blocking commands, with corrupt-session control.
- [x] Recover without overwriting originals; surface paths and save recovery settings.
- [x] Test recovery/restart and preserved damaged/future data; commit.

## Task 3: File claims and shared previews (audit 3, 4)
- [x] Reproduce cancel/retry and folder-switch empty-file conflicts.
- [x] Reuse pinned owned empty claims safely; reject changed/replaced user files.
- [x] Measure snapshot sharing; use Arc and test stable identity across ticks.
- [x] Test and commit.

## Task 4: Removal and pause consistency (audit 5, 6)
- [x] Fault-inject persistence/engine failures; keep rows visible and recoverable.
- [x] Clean owned metainfo only after durable successful removal; keep payloads.
- [x] Test pause/remove/re-add scenarios and commit.

## Task 5: CI, readability and IPC (audit 8–10)
- [x] Inspect primary cargo-deny documentation; run and configure scans honestly.
- [x] Add pinned scan workflow and update monitoring; document policy findings.
- [x] Expand async blocks, share row height, stabilize and protect IPC namespace.
- [x] Test IPC safety/forwarding; commit.

## Task 6: Delivery
- [x] Full Rust/decoder/Python checks and Mac lifecycle probe; one independent review.
- [x] Fix important review findings with regression tests; rebuild local app.
- [x] Record all ten dispositions and actual verification, leave app ready.

Review focus: corrupt/future session files never overwritten; recovery files
cannot overwrite corrupt alternates; reclaiming does not overwrite replaced or
nonempty files; persistence/engine failure cannot hide registered jobs; no user
payload deletion; multiple preview ticks must share large data; IPC remains
private and cross-process compatible; scan exceptions must have explicit evidence.
