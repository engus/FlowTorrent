# Memory Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce measured avoidable memory without changing downloads or Windows behavior.
**Architecture:** Retain Slint software/Winit and librqbit. Avoid an unnecessary
metadata AST; retain Mac allocator reclamation only if measured useful.
**Tech Stack:** Rust 1.92.0, Slint 1.18.1, librqbit 9.0.1; existing Python probes.
**Spec:** `docs/superpowers/specs/2026-10-22-memory-optimization-design.md`.

## Global Constraints

- Shared Windows x64, Mac ARM and Mac Intel code; macOS minimum 14.0.
- Pinned dependencies; `--locked`, local `-j 2`; no public discovery in test prerequisites.
- Preserve path capabilities, no payload deletion/overwrites, durable registry intent.
- Four settings, existing rendering quality, throughput and discovery behavior.
- Reuse attached worktree; no publication or merge requested.

## Review Focus

- Pure v2/hybrid/unknown fields must retain valid classification without an AST.
- Hostile/deep metadata must remain bounded in both inspection and magnet paths.
- Background reclamation must not block UI, free live data or run on every tick.
- Rapid hide/show, native picker parenting, Dock reopen and Quit must remain safe.
- Memory results must separate RSS/footprint, warmup, scale and transfer progress.

## Task 1: Bounded format probe

Files: `src/engine/librqbit.rs`, `src/engine/metainfo.rs`,
`src/ui/models.rs`, `src/ui/controller.rs`, `tests/ui_models.rs`,
`tests/metadata_memory.rs`.

- [x] Add a test-only allocation counter and a valid v1 torrent with a large ignored extension list. Assert source bytes/ID preserved and inspection peak additional allocations below 8 MiB. Run it and observe the current full-tree allocation fail.
- [x] Replace generic tree construction with borrowed serde format fields and ignored values; preserve the existing nesting check and metadata limits.
- [x] Add a selection setup budget for 10,000 files; observe 2,885,536 extra bytes, then share preview data and use a boolean vector (10,000 extra bytes). Verify sparse IDs, padding and release of the shared owner.
- [x] Run allocation regression, add-torrent and magnet tests; commit.

## Task 2: Measured Mac reclamation and attribution

Files: `examples/memory_breakdown.rs`, optional `src/platform/memory.rs`,
`src/ui/controller.rs`, `examples/resources/mod.rs`.

- [x] Complete source-guided reclamation experiment and record the result. If ineffective, document rejection without product changes.
- [x] Reclamation proved ineffective (zero bytes released); reject it and window destruction. Keep Mac/Windows product platform code unchanged.
- [x] Polish attribution output and verify release execution. Include actual scale/window geometry and hide behavior in reports.

## Task 3: Verification and delivery

- [x] Run fmt, all-target Clippy/tests, decoder tests, Python policy/packaging tests, dependency scan and Mac native lifecycle probe.
- [x] One fresh independent branch review; fix Important/Critical findings once with regressions and rerun required checks.
- [x] Run comparable release empty, active5, hidden5, paused20 and cycles5 probes; retain raw CSV/environment and report meaningful progress and CPU.
- [x] Rebuild local `.app`, verify background/reopen/Quit, record observed benefits and limits, commit documentation and leave app ready.
