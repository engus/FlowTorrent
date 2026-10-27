# Dependency checks

CI uses cargo-deny 0.20.2 via an immutable action revision to check advisories,
licenses and sources against Cargo.lock for Windows x64 and both Mac targets.
It also saves Cargo's exact package/feature/license metadata as an artifact.
Dependencies and Actions have weekly Dependabot updates; these produce reviewed
PRs, never automatic merges or releases. Pinned core upgrades still need their
own change and platform verification.

The allowlist accepts the common permissive licenses actually used by this
graph. Slint's existing Royalty-free 2.0 choice is allowed only for its named
1.18.1 crates; the visible attribution badge remains required. option-ext 0.2.0
has a specific MPL-2.0 allowance. Its upstream source is unchanged; preserve
its license and source availability in release notices. This policy check and
metadata inventory do not replace a complete distributable license bundle.

Configuration semantics: [official cargo-deny license configuration](https://embarkstudios.github.io/cargo-deny/checks/licenses/cfg.html).

## Reviewed advisory exception

The 2026-10-22 scan found
[RUSTSEC-2026-0293](https://rustsec.org/advisories/RUSTSEC-2026-0293) in ringbuf 0.4.8,
through librqbit-utp 0.7.0. The reported double-drop requires an element's
destructor to panic during Consumer::skip/clear. In this locked consumer,
`src/stream_tx.rs` defines `type RingBuf = SharedRb<Heap<u8>>`; all ringbuf use is
through that byte buffer. `u8` has no destructor, so the reported prerequisite
does not occur in this application. This is an assessment of the current
consumer, not a claim that ringbuf 0.4.8 is generally fixed.

The exception names only this advisory and has no blanket vulnerability
suppression. `scripts/check_ringbuf_exception.py` checks immutable registry
versions and the exact direct-consumer graph. CI fails for any different
ringbuf version/source or additional/different consumer, requiring re-evaluation.
Remove the exception when upstream UTP supports the patched ringbuf release;
the advisory identifies >=0.5.2 as fixed. No pinned librqbit upgrade was made.

The scan also found yanked Tokio 1.54.0. It was replaced with non-yanked 1.54.1;
the published dependency requirements are identical, and its Rust 1.85 minimum
fits this repository's 1.92 toolchain. Full regression verification is recorded
in the audit follow-up.
