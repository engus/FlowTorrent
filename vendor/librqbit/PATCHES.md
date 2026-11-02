# FlowTorrent file scheduling patch

Exact crates.io librqbit **9.0.1**, archive SHA256
`4349c0f35ede576a87cc2881dc6eb286ff7a1c8fc4d1f7fef95b5c5a530a7174`.
Upstream revision: a499d2f243d124e144aef137afe7cb304a6e3f36, crates/librqbit.
Original licenses and copyright are retained. No version/dependency upgrade.
The package archive's standalone Cargo.lock is omitted; the root workspace lock
owns the application graph. Normalized Cargo.toml and other sources are unchanged.

File-order changes are in src/session.rs, src/torrent_state/mod.rs and
src/torrent_state/live/mod.rs:

- Optional complete file order in AddTorrentOptions, validated before storage
  creation/transfer. No override preserves upstream filename ordering.
- Paused/paused-initializing handle setter validates a complete permutation,
  holds the managed-state read lock against concurrent unpause, then replaces
  an optional shared order. No live scheduler mutation or tracker lock inversion.
- Existing Live constructor reads this order, retained through pause/check/resume.
  Existing ChunkTracker/PieceTracker use it when reserving available queued pieces.

Application Low/Normal/High mapping lives in the adapter, not this crate. Ranking alone does not cancel in-flight requests. The application pauses the
torrent before applying an edit; pause can cancel/requeue incomplete chunks,
while preserving verified pieces. Missing high-ranked peer pieces fall through
to lower available pieces via the existing peer_has_piece filter. Shared selected
pieces are encountered through the highest-ranked owning selected file.

The torrent-priority extension additionally changes src/lib.rs and src/limits.rs,
and adds src/download_queue.rs. Optional initial download weight (default 2) and a
paused setter retain the relative preference across check/pause/resume. Session
request admission uses a cancellable packetized weighted fair queue at the shared
download cap. All peers of a torrent share its virtual clock; idle flows retain
no credit. The token bucket retains the one-second initial burst, wakes on rate
changes and bypasses waiting when unlimited. No scheduler task is spawned. The
adapter maps Low/Normal/High to 1/2/4, independently of per-file ordering.

Payload storage, network privacy, selection, piece verification and upload code
are unchanged. Shared download request admission is intentionally changed; this
is not a protocol, tracker or transport change.
Reassess/remove the patch when upstream offers a verified equivalent public API.
Application integration tests cover actual scheduling and native order validation.
