# Claude audit follow-up

Date: 22 October 2026. Original report: [claude-audit.md](../claude-audit.md).
Work continues from the existing local Mac and magnet implementation, on
`codex/claude-audit-fixes`. The original report is preserved as written.

## Disposition of the ten findings

| Finding | Result |
| --- | --- |
| 1. Shutdown described as startup failure | Startup, runtime and shutdown failures have separate context and localized headings. Shutdown timeouts retain their shutdown classification. |
| 2. Damaged preferences block the app | Use defaults or a valid `settings.recovered.json`, show the original/recovery paths, and save corrections separately. Preserve damaged/future originals and damaged recovery files. A damaged session deliberately remains read-only, with its path and recovery guidance visible. |
| 3. Abandoned empty claims block another add | Reuse this process's pinned, unchanged empty claims across cancellation and destination changes. Validate file identity, size and hard links. External writes/replacements still fail safely. No payload files are deleted. |
| 4. Preview cloned every tick | Controller, watch snapshots, pending adds and UI share an `Arc<TorrentPreview>`. A large-metadata regression verifies that ticks retain the same allocation. |
| 5. Removed torrents leave metainfo cache | After engine removal and durable registry removal, delete only the saved application-owned metainfo whose parsed ID matches the removed torrent. Preserve damaged/mismatching files; log cleanup failures. Downloaded payloads and source torrents remain untouched. |
| 6. Engine and saved intent drift | Persist pause/stop intent first and retain the registry row until engine removal and the final save succeed. Retry reconciles actual state with durable intent, including already-paused/live engines and retained initial verification. |
| 7. Early errors unlogged; excessive arguments generic | Record failures even before tracing initialization, with stderr fallback. More than 64 files gets a specific localized limit error. |
| 8. Dependency/license checks missing | Add pinned cargo-deny advisory/license/source checks, a saved dependency inventory and weekly Dependabot PRs. Replace yanked Tokio 1.54.0 with 1.54.1 in a separate dependency change. See the explicit advisory exception below. |
| 9. Compressed blocks and duplicated row geometry | Expand command/UI async blocks and dialog setup. Read scroll geometry from Slint's row-height property; a changed-height regression covers it. |
| 10. Predictable shared temp parent and unstable hash | Use a fixed FNV-1a namespace protocol and golden test. On macOS, create the socket directory inside the OS-provided private per-user temporary root, checking ownership, permissions and links. Windows retains its owner-only pipe DACL. |

## Limits retained deliberately

- Empty-claim reuse lasts for the current process. Handles remain open until
  exit; abandoned placeholders may still conflict after restarting. They are
  never deleted or adopted merely because they are empty.
- Corrupt session repair is not automated. Registry contents are the authority
  for which existing payloads the app owns, so damaged/future manifests remain
  protected from writes.
- Metainfo cleanup is best effort. A failed cleanup does not resurrect a removed
  row, and unexpected cache content is preserved for investigation.
- The private OS temporary parent addresses the supported Mac target. The
  existing non-Mac Unix fallback still uses its temporary directory; Linux is
  outside the release matrix.

## Dependency findings

The actual scan found [RUSTSEC-2026-0293](https://rustsec.org/advisories/RUSTSEC-2026-0293)
in ringbuf 0.4.8. Its panicking-destructor prerequisite is absent from the locked
librqbit-utp 0.7.0 consumer, which exclusively uses `SharedRb<Heap<u8>>`.
The exception is limited to that advisory; CI checks the exact immutable
registry versions and direct-consumer graph before applying it. This is an
assessment of the present use, not a general fix to ringbuf. Remove/reassess
the exception when the graph changes. [Dependency policy](dependency-policy.md)
records the evidence and license allowances.

License policy checks and the metadata inventory do not replace the complete
license/source notices required before distribution. Slint's visible attribution
is retained. No pinned librqbit or Slint upgrade was made.

## Verification and local delivery

Completed on this arm64 Mac with Rust 1.92.0 and deployment target 14.0:

- `cargo fmt --all -- --check` and Clippy on all targets with warnings denied.
- `cargo test --locked --all-targets -j 2`: **108 passed**, zero failures or ignored tests.
- `cargo test --locked -p librqbit-bencode -j 2`: **18 passed**.
- Python packaging and dependency policy suite: **9 passed**.
- cargo-deny 0.20.2: advisories, licenses and sources pass under the documented
  policy; locked Cargo metadata passes the ringbuf exception scope guard.
- Native lifecycle probe: Winit delegate identity, ordered AppKit file callbacks,
  reopen callback and hidden event loop **PASS**.

The release app was rebuilt and ad hoc signed at
`target/local-macos/FlowTorrent.app` in the original project folder. Bundle
identity, arm64 architecture, macOS 14.0 minimum, system-only dynamic libraries
and strict signature verification passed. Executable code/data sections match
the release output (signing changes the Mach-O signature metadata).

GUI smoke: launched the bundle, opened/cancelled the Add torrent file picker,
opened the magnet pane with empty submission disabled and malformed input
rejected, opened/cancelled Settings, quit with Cmd+Q, confirmed process exit,
and relaunched to one running process. The app is left open for testing.
Preference-corruption behavior was verified with isolated automated fixtures;
no live user settings or session files were deliberately damaged.

Tests use controlled local swarms and fixtures, without public discovery as a
test requirement. The one independent source review found two recovery edge
cases; regressions reproduced both before their fixes.

Windows x64 and Mac Intel CI remain configured, but their runs and physical
platform checks have not been performed for this local branch. The Mac app is
a local arm64 test bundle with ad hoc signing, not a notarized distribution.
No release, tag, installer upload or remote publication was performed.
