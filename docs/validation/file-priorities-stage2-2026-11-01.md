[Русская версия](file-priorities-stage2-2026-11-01-ru.md)

# File priorities, stage 2 — 1 November 2026

Status: stage 2 implemented, reviewed and locally verified on Mac ARM. Windows/Intel CI remains open. No commit/push/merge/publication.

Branch `experiments/file-priorities`; base `142f2bb75a4c4c8769f15a44d0f4fface33406b0` plus uncommitted stages 1/2. Stage 1 is backed up in ignored `_work/validation/file-priorities-20261101/stage1-working-files.tar.gz`. Main, AppKit and other experiments were not edited.

[Behavior](../architecture/file-priorities-experiment.md) · [Plan](../superpowers/plans/2026-11-01-file-priorities.md) · [Stage 1](file-selection-stage1-2026-10-31.md)

## Behavior and patch

Per-file Low / Normal / High dropdown, localized RU/EN, next to the checkbox or on a second line in narrow windows. Normal is the default. Exclusion retains priority; configuring an excluded file does not enable it. Stable IDs and checking/error/busy gates protect edits; failed commands/sends restore canonical widget values.

New piece reservations prefer selected High, Normal, Low owners, filename order within a tier. Disabled owners cannot promote shared pieces; missing High pieces do not block available Low pieces. All selected files still count toward completion. This is not separate bandwidth allocation or a queue between torrents. An edit briefly pauses/reconnects the torrent and can re-request unfinished chunks; verified pieces are retained. Steady-state ranking does not itself cancel requests.

Sparse overrides use manifest v3; v1/v2 load as Normal, checkbox edits never downgrade v3. Restore configures the native order before unpause. Invalid tiers/duplicates/padding/unknown IDs and damaged/future state are rejected. Initial controller adds remain Normal-only; prefilled overrides are rejected before claims to prevent an unpersisted runtime policy.

`vendor/librqbit` is exact 9.0.1, archive SHA256 `4349c0f35ede576a87cc2881dc6eb286ff7a1c8fc4d1f7fef95b5c5a530a7174`, original licenses retained. Only three upstream Rust files differ: session.rs, torrent_state/mod.rs and torrent_state/live/mod.rs. [Patch notes](../../vendor/librqbit/PATCHES.md). No versions upgraded; Cargo.lock only replaces registry source/checksum with the local source. Storage/network/privacy/native platform code and LSD/bencode patches remain unchanged. Engine persistence/fastresume stay off; notification intent is not reset.

## Verification

Rust 1.92.0, locked, jobs 2, Mac deployment 14.0, isolated target; controlled loopback swarms. Raw logs, ledger and 367 frozen source/build-input hashes: ignored `_work/validation/file-priorities-20261101/`.

- Pre-review full run: 184 passed, 0 failed/ignored. Its release was deliberately interrupted after review findings; historical, not final evidence.
- Final fmt, native-patch rustfmt and strict all-target Clippy: passed.
- Final all-target tests: **187 passed, 0 failed, 0 ignored** (41 harness summaries).
- Standard release: failed loading `zerofrom_derive` (E0463/E0432), as on stage 1; error log preserved. Release with local `--config 'profile.release.build-override.strip="none"'`: **passed**, 2m 03s. Cargo profile files were not changed for the workaround.
- `cargo run --locked --example macos_lifecycle_probe -j 2`: **passed** (delegate identity, native file callback ordering, reopen/modal lease and hidden event loop).
- Locked offline dependency metadata and `check_ringbuf_exception.py`: **passed**; librqbit resolves to local 9.0.1, versions unchanged.

All 367 frozen inputs remained byte-identical through final verification. Release executable SHA256: `cb8ca13c9973e52b831b22f9a4ce5d0aa604e4ba1b6357b6df93c564828604cf`. Exit codes/source hashes are retained with `completed.json`.

A fresh review found P1: completion bookkeeping could overwrite unresolved third intent while read-only. A reproducing injected-completion test failed before correction and passed after. Read-only also blocks central saves and automatic stale Checking resume (separate RED/GREEN test). Initial-add priority/manifest divergence was independently reproduced and fixed by explicit rejection. Review P2 corrected inaccurate no-preemption wording to describe paused edits. All six priority-controller tests pass, including exclusion/restart/schema retention, before/after-replacement save errors and engine failure/Retry. These fault injections are not network-transfer evidence.

Real transfers demonstrate High-first preference, reversal, full payload equality and unavailable-High/available-Low fallback. The latter explicitly verifies zero checked High bytes at the seed; the initial all-valid seed was an invalid control and is not evidence. Order-construction tests cover ties, invalid overrides and excluded-owner shared-piece ranking. Rendered Slint tests cover wide/narrow interaction, busy gating and rejected-send rollback.

## Limits

No Windows x64/Mac Intel CI or physical native dropdown test. Unchanged platform source is not runtime evidence. Mac lifecycle is integration evidence, not a manual Finder/Dock/DPI check. No separate shared-piece network reservation trace, restored-swarm ordering study, concurrent setter/unpause stress test or resource benchmark was run. No release, installer, tag, commit or push was published.
