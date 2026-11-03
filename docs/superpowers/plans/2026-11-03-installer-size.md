[Русская версия](2026-11-03-installer-size-ru.md)

# Installer Size Experiment Implementation Plan

> **For agentic workers:** Execute this authorized research spike inline; preserve the worktree and evidence. No promotion or publication.

**Goal:** Measure smaller installers without removing functions and compare controlled runtime resources with release.
**Architecture:** Add a separate compact profile and explicit profile selection in packaging; retain release unchanged. Produce strongly compressed release and compact packages. Extend the resource entry point with same-runner paired measurements, retaining single-profile mode.
**Tech Stack:** Rust 1.92, pinned Slint/librqbit, Python, hdiutil, Inno Setup, existing GitHub Actions.
**Spec:** [Research design](../specs/2026-11-03-installer-size-design.md).

## Global Constraints

- All dependencies, Cargo.lock, features, production Rust/UI/assets, panic semantics and complete licenses stay unchanged.
- Always --locked and -j 2; macOS deployment target 14.0; unsigned/ad-hoc test artifacts only.
- No Release, tags, main merge or installer publication. Keep original state and artifacts.
- Comparisons use the same runner and metric, A/B/B/A and controlled local swarms. Record limitations.

## Review Focus

- Custom profile must select the matching output binary rather than silently packaging release.
- Stronger compression must preserve bundle bytes and complete notices, signature and architecture.
- Summaries must reject missing samples, hidden/visible mistakes, reversed CPU time/progress and absent transfer.
- CPU percentages near zero and noisy repeat ranges must not become invented speedups.
- Probe source/profiles differ from installed Windows CRT; identify this explicitly.

### Task 1: Build and package variants

- [ ] Write failing profile/output/manifest and real native compressed-image preservation tests; run RED.
- [ ] Add compact profile; profile-aware packagers; stronger compression and explicit metadata; generate release-packed and compact packages using existing workflow.
- [ ] Run portable suite GREEN; commit small build change.

### Task 2: Paired resource comparison

- [ ] Write failing CSV validity, missing metrics, ABBA and comparison tests using real sample files; run RED.
- [ ] Refactor existing sample analysis; add paired runner and profile metadata. Preserve single-profile mode, scale reporting and artifact paths.
- [ ] Test compact profile/all targets and Mac lifecycle on CI before paired probes. Preserve raw observations in existing artifact globs.
- [ ] Run Python suite GREEN; commit tooling.

### Task 3: Native builds and measured report

- [ ] Push only experimental branch; run Check, test packages with artifacts, release validation. Baseline main resource run 41605617329 is already dispatched.
- [ ] Verify all results; download to ignored paths and check hashes, bundle contents and native installer smoke.
- [ ] Compare old/release-packed/compact sizes and within-runner CPU/memory/progress ranges. Document failures and unavailable measurements honestly.
- [ ] Review complete diff and record recommendation in paired EN/RU evidence; leave branch/worktree available, no merge/publication.
