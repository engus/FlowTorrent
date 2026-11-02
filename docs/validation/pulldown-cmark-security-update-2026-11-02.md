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

Local cargo-deny 0.20.2 now reports `advisories ok, licenses ok, sources ok`.
The existing non-blocking yanked-version warning for `clipboard-win 5.4.1` remains;
this change does not expand into unrelated dependency updates.

Local Mac ARM checkpoint after the update: fmt, strict all-target Clippy and all
199 Rust tests (44 harnesses, no failures or ignored tests) passed. Release and
three-platform CI evidence are separate from this checkpoint.

Raw metadata, crate checksum evidence and logs are retained under ignored
`_work/validation/file-priorities-ci-20261102/`. Full platform CI must be evaluated
for the corrected commit, not the interrupted first attempts. Installer uploads
remain disabled; no main/AppKit changes, release, tags or publication.
