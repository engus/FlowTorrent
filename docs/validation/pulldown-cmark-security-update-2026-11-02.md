[Русская версия](pulldown-cmark-security-update-2026-11-02-ru.md)

# Transitive parser security update — 2 November 2026

The first full CI attempt for `experiments/file-priorities`, sources
`10d647b94eab90e2b998a5318fef444816ea6710`, failed its dependency gate:
[Check 41416575744](https://github.com/engus/FlowTorrent/actions/runs/41416575744).
It reported [RUSTSEC-2026-0414](https://rustsec.org/advisories/RUSTSEC-2026-0414.html)
in the existing Slint transitive dependency `pulldown-cmark 0.13.4`. The advisory
concerns exponential event growth when parsing Markdown with wikilinks enabled.
This records the dependency finding, not a demonstrated exploit of FlowTorrent.

The separate correction changes only that package's version and checksum in
`Cargo.lock`, to upstream `0.13.5`. The published crate archive was checked against
its crates.io checksum; its normalized manifest differs only in version. Locked
Cargo metadata accepts the graph. Direct pinned dependencies, Slint 1.18.1,
librqbit 9.0.1, the toolchain and application sources are unchanged by this fix.
No advisory exception or CI check was relaxed.

At the first checkpoint, local cargo-deny 0.20.2 reported `advisories ok, licenses ok, sources ok`.
The existing non-blocking yanked-version warning for `clipboard-win 5.4.1` remains;
this change does not expand into unrelated dependency updates.

Local Mac ARM checkpoint after the update: fmt, strict all-target Clippy and all
199 Rust tests (44 harnesses, no failures or ignored tests) passed. Release and
three-platform CI evidence are separate from this checkpoint.

Raw metadata, crate checksum evidence and logs are retained under ignored
`_work/validation/file-priorities-ci-20261102/`. Full platform CI must be evaluated
for the corrected commit, not the interrupted first attempts. Installer uploads
remain disabled; no main/AppKit changes, release, tags or publication.

## Pre-merge follow-up: 0.13.6

The full branch checks above subsequently passed, but the fresh
[PR #9 check](https://github.com/engus/FlowTorrent/actions/runs/41441117242)
reported the additional [RUSTSEC-2026-0433](https://rustsec.org/advisories/RUSTSEC-2026-0433.html)
for 0.13.5. This concerns quadratic parsing of nested image wikilinks with
`ENABLE_WIKILINKS`; [upstream 0.13.6](https://github.com/pulldown-cmark/pulldown-cmark/releases/tag/v0.13.6) fixes it.

The follow-up changes only the package version/checksum to 0.13.6. Its published
archive checksum was verified and its normalized manifest differs from 0.13.5
only in version. Locked metadata accepts the graph and cargo-deny again reports
`advisories ok, licenses ok, sources ok`. No advisory exception or CI rule changed;
Slint 1.18.1, librqbit 9.0.1 and application sources remain unchanged.

The earlier 0.13.5 test/package evidence remains historical. Fresh checks for the
corrected PR head are required before merge. Raw evidence is retained under ignored
`_work/validation/file-priorities-merge-20261102/`.

Fresh local Mac ARM checkpoint for 0.13.6: fmt, strict all-target Clippy and all
199 Rust tests (44 harnesses, no failures or ignored tests) passed. Release and
three-platform PR checks are evaluated separately.
