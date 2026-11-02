[Русская версия](file-selection-stage1-2026-10-31-ru.md)

# File selection, stage 1 — 31 October 2026

Status: stage 1 implemented, reviewed and locally verified on Mac ARM. Release succeeded with the local build override documented below; Windows/Intel verification remains open.

Branch `experiments/file-priorities`, isolated worktree `file-priorities/FlowTorrent`. Base commit `142f2bb75a4c4c8769f15a44d0f4fface33406b0`; changes remain uncommitted. The tested source is the base plus this working diff, not main or the Slint performance candidate. No AppKit-prototype, dependency, installer, release or tag changes.

[Approved experiment](../architecture/file-selection-experiment.md) · [Plan](../superpowers/plans/2026-10-31-file-selection.md)

## Behavior

- Per-file checkboxes in the single-torrent details drawer; edits disabled while checking/error recovery/busy. Stable file IDs and torrent identity are checked again by the controller.
- Individual commands compose against the latest manifest. Pause only that torrent, persist intent, update librqbit's paused wanted set, and resume only when desired state is active. Completed selections can enable previously excluded files and reopen the existing pinned writers.
- Disabling preserves payload and verified pieces. Shared boundary pieces can contain bytes from excluded files. An empty selection is retained as “No files selected”, with zero selected-byte progress, no completion notification and no “Downloaded” ETA copy. Initial add still requires at least one file.
- Manifest version 1 remains readable; a selection edit upgrades to version 2, including empty selection. Older clients must reject it without overwriting. Restore remains paused until checking finishes. Notification intent is never reset: at most once per torrent.
- Before-write save failure retains the old policy. An uncertain post-replacement error is reconciled with disk; a newly observed selection stays paused in recovery, and Retry re-establishes durability before restoring it. Unknown/damaged state blocks further writes until restart/repair. Engine update failure similarly cannot resume an obsolete policy.
- Remove from list keeps payload. Trash includes all pinned, claimed non-padding files, including disabled files and owned empty/boundary data, but retains identity/hard-link checks and foreign files. Recovery markers clear on successful removal/re-add.

## Evidence

Rust 1.92.0, locked dependencies, local parallelism 2, `MACOSX_DEPLOYMENT_TARGET=14.0`, isolated target directory. All fixtures use controlled loopback swarms; no public trackers or public discovery are needed.

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --locked --all-targets -j 2 -- -D warnings`: passed.
- `cargo test --locked --all-targets -j 2`: **176 passed, 0 failed, 0 ignored**, 38 harness summaries. This includes 21 library tests, local real transfer/restore/path safety and the new selection suites.
- Standard `cargo build --locked --release -j 2`: failed before application compilation with E0463/E0432 loading `zerofrom_derive` on this Mac. The local-only retry with `--config 'profile.release.build-override.strip="none"'` **passed** (4m 56s). This previously used Mac build workaround preserves the proc-macro build outputs without changing the application source/profile in Git. No Cargo manifest/lock changes were used to bypass the failure.
- `cargo run --locked --example macos_lifecycle_probe -j 2`: **passed** — delegate identity, ordered native file callbacks, reopen, native modal lease and hidden event loop.

Raw logs and the execution ledger are under ignored `_work/validation/file-selection-20261031/`. Source fingerprint file SHA256: `58c849b4b22fa91d30c39321035707f4185610f5b457b2ba98c6ca8aee5f593e` (126 source/build/test inputs). All 126 inputs were independently confirmed unchanged at completion. Release executable SHA256: `b15fa10b4bb26b8e90da5e3bdfa39a2baa584d7faa3509b336413e477abc8a07`. Final check exit codes are recorded in ignored `completed.json`; the original failed default-release log is preserved.

## Review and regression tests

A fresh read-only whole-branch review found four P2 issues, all reproduced before correction: paused empty removal, stale recovery marker after re-add, post-replace save ambiguity, and empty “Downloaded” projection. Their covering tests passed after fixes. A rendered 1280×720 Slint widget test additionally reproduced a failed-enqueue checkbox remaining locally unchecked; it now reverts to the canonical selection, and disabled controls dispatch nothing. No second review or unverified reviewer success claim substitutes for tests.

Other new evidence covers serialized rapid toggles, invalid IDs/duplicates, active and paused intent, failed saves, error Retry, empty restart, completion suppression, checked-data reuse, shared boundary pieces, live rate-limited deselection/re-enable, and Trash retaining a foreign file.

## Limits and remaining work

No Windows x64 or Mac Intel CI/runtime or physical checkbox check was run. Common Rust/Slint code uses the pinned engine API and existing capability-based handles; this is source compatibility work, not platform-runtime evidence. The lifecycle probe is an integration check, not a Finder/Dock/Retina manual check. No long-run resource/large-public-swarm claims follow from these fixtures. Priority scheduling remains stage 2 and is not simulated by these checkboxes. No commit, push, merge or publication is performed.
