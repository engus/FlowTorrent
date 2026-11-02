[Русская версия](2026-11-01-file-priorities-ru.md)

# File priorities implementation plan

> For agentic workers: execute inline with superpowers:executing-plans and test-driven-development; one fresh final whole-branch review. Preserve work; do not commit/push/merge.

**Goal:** real per-file Low / Normal / High with durable restore and Slint control.
**Architecture:** sparse application overrides -> stable complete order -> small pinned librqbit vendor patch -> existing piece scheduler. Reuse stage 1 durable paused policy transaction.
**Tech:** Rust 1.92.0, librqbit 9.0.1, Slint 1.18.1, Mac deployment 14.0, locked dependencies and -j2.
**Spec:** [approved two-stage extension](../../architecture/file-priorities-experiment.md).

- [x] Storage/domain RED then GREEN: v3 roundtrip, legacy Normal, duplicate/invalid/future preservation.
- [x] Backend RED then GREEN: exact vendor provenance, complete-order validation and pause/restore retention; real High-first / Low-completes test, equal-tier and availability behavior.
- [x] Controller RED then GREEN: durable priority commands reuse policy transaction; selection never downgrades schema, excluded priority retained, restore before unpause, engine/save errors and retry.
- [x] UI RED then GREEN: localized per-file dropdown, real IDs, busy/check/error gates and rejected-command rollback.
- [x] fmt, strict Clippy, all-target tests, release and Mac lifecycle; fresh review, reproducing fixes and paired evidence. No main/AppKit/other-experiment writes.

Review focus: old manifests/future/corrupt fields; shared pieces and absent high pieces; stale ID/dropdown rejection; active/paused/initializing lock order; ambiguous saves/retry/removal/re-add; priorities survive toggling and checking; unchanged pinned storage, private swarms and platform boundaries.

## Verified outcome

187/187 test entries, 0 ignored; fmt, native patch formatting, strict Clippy, release with documented local proc-macro override and Mac lifecycle passed. Fresh review findings reproduced/fixed; native scheduling, availability, widget rollback and read-only recovery covered. [Evidence](../../validation/file-priorities-stage2-2026-11-01.md). Windows/Intel CI and physical checks remain open. Worktree preserved uncommitted; no push/merge/publication.
