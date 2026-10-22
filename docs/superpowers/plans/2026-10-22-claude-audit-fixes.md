# Claude audit implementation plan

Execute inline with superpowers:executing-plans. Spec:
`docs/superpowers/specs/2026-10-22-claude-audit-fixes-design.md`.
Baseline: local Mac and magnet work committed before these fixes.

## Task 1: Lifecycle diagnostics (audit 1, 7)
- [ ] Reproduce shutdown timeout classification and early logging failures.
- [ ] Separate lifecycle error context, log early failures, show useful argument errors.
- [ ] Test and commit the focused fix.

## Task 2: Settings recovery (audit 2)
- [ ] Reproduce corrupt-settings blocking commands, with corrupt-session control.
- [ ] Recover without overwriting originals; surface paths and save recovery settings.
- [ ] Test recovery/restart and preserved damaged/future data; commit.

## Task 3: File claims and shared previews (audit 3, 4)
- [ ] Reproduce cancel/retry and folder-switch empty-file conflicts.
- [ ] Reuse pinned owned empty claims safely; reject changed/replaced user files.
- [ ] Measure snapshot sharing; use Arc and test stable identity across ticks.
- [ ] Test and commit.

## Task 4: Removal and pause consistency (audit 5, 6)
- [ ] Fault-inject persistence/engine failures; keep rows visible and recoverable.
- [ ] Clean owned metainfo only after durable successful removal; keep payloads.
- [ ] Test pause/remove/re-add scenarios and commit.

## Task 5: CI, readability and IPC (audit 8–10)
- [ ] Inspect primary cargo-deny documentation; run and configure scans honestly.
- [ ] Add pinned scan workflow and update monitoring; document policy findings.
- [ ] Expand async blocks, share row height, stabilize and protect IPC namespace.
- [ ] Test IPC safety/forwarding; commit.

## Task 6: Delivery
- [ ] Full Rust/decoder/Python checks and Mac lifecycle probe; one independent review.
- [ ] Fix important review findings with regression tests; rebuild local app.
- [ ] Record all ten dispositions and actual verification, leave app ready.

Review focus: corrupt/future session files never overwritten; recovery files
cannot overwrite corrupt alternates; reclaiming does not overwrite replaced or
nonempty files; persistence/engine failure cannot hide registered jobs; no user
payload deletion; multiple preview ticks must share large data; IPC remains
private and cross-process compatible; scan exceptions must have explicit evidence.
