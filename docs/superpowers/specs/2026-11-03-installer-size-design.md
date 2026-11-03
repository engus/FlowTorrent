[Русская версия](2026-11-03-installer-size-design-ru.md)

# Installer size research spike

The maintainer requested an experiment on a separate branch, preserving all functions, comparing size and performance, without publication. This is an experimental spike, not approval to promote an optimization into main.

Baseline: main 9714c4bf0129988977746a3c469b6e843340d9dd; downloaded test packages and exact hashes from Actions 41516995085. Keep production Rust/UI/assets, Cargo.lock, dependency features, panic behavior, platform minimums and complete notices unchanged.

## Approaches and experiment choices

1. Stronger package compression, retaining the ordinary release executable: ULMO/LZMA DMG and solid LZMA2/ultra64 Inno. This isolates package/download gains from executable changes.
2. A separate compact Cargo profile inheriting release: opt-level=s, fat LTO, strip=symbols, one codegen unit. Compare its packages with both the old packages and freshly rebuilt release packages using stronger compression. These are the experimenter's settings to measure, not a promised optimum.
3. Removing features/dependencies, changing panic behavior, executable packers or downloading runtime components was rejected: outside the requested preservation boundary.

## Measurement

Use identical source and dependencies. Record profile, compression, source/lock hashes, artifact bytes and SHA-256. Both package variants retain all notices, identity and test-build labels. Existing installers are never overwritten or installed into the maintainer's profile.

On each native CI runner, compare release and compact resource probes sequentially in A/B/B/A order, two observations per profile per scenario. Keep five scenarios, software renderer, actual scale, 20-second warmup and 30 samples per observation; save raw CSV/environment and per-repeat CPU, native memory and verified progress. Transfer rate is controlled at 655360 B/s: this tests CPU/memory under matched work, not maximum network throughput. Require every active observation rate to stay within 20% of the median release rate before emitting a CPU ratio; otherwise label the work unmatched and preserve raw measurements. This tolerance accounts for short sample-window/status-update granularity, not a pass threshold for regressions. Compare profiles only within the same runner, metric and scenario; report ranges and noise, not statistical significance. Windows resource probes retain the existing dynamic CRT while installers use static CRT; report this instrumentation limit explicitly. No physical acceptance or overnight claim.

Release defaults remain byte-for-byte unchanged. CI performs existing debug checks plus all-target compact-profile tests and native Mac lifecycle checks. Packaging runs native signature/architecture/license/version checks and disposable Windows preservation smoke. No release, tags, main merge or website publication. Test artifacts may be saved for review, as in the authorized baseline build.

## Isolation and evidence

Branch experiments/installer-size in a managed worktree. Original main and uncommitted reports stay untouched. Local Cargo is unavailable and free disk space is limited; Rust checks run on CI. Existing workflow files need no edits: branch-only packaging defaults produce comparison artifacts, and the resource-validation entry point invokes the paired experiment. Keep ordinary single-profile resource validation available with an explicit opt-out. No authorization to adopt a result into main is inferred.

Operational refinement: the 15-sample pilot hit the old 60-minute CI limit during compilation, while Windows completed all 20 main observations and 199 compact-profile tests before a redundant scale rebuild. The experiment now uses 150 minutes, a release probe prebuilt by the workflow, explicit reuse for scale checks, streamed compiler logs and 30-sample main windows. Ratios below a 0.1% release CPU baseline are suppressed; absolute differences and repeat ranges remain. Full compact tests remain mandatory for a successful main run, after measurements so a timeout does not erase useful observations.
