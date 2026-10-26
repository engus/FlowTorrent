# FlowTorrent patch: bounded recursive decoding

Source: librqbit-bencode 9.0.1, crates.io archive, rqbit commit
`a499d2f243d124e144aef137afe7cb304a6e3f36`, `crates/bencode`.
Original Apache-2.0 license and copyright are retained in LICENSE and LICENSE-APACHE.

FlowTorrent changes (2026-10-22): track container depth in the deserializer and
reject entry beyond 128 lists/dictionaries, including ignored unknown fields.
This protects the upstream peer-metadata decode that happens before metadata
can reach FlowTorrent's own nonrecursive file guard. Counters unwind on both
successful and failed visits. The guard also applies to from_bytes_with_rest,
while leaving trailing binary protocol payloads untouched.

Regression coverage: tests/magnet.rs uses a loopback BEP 9 peer serving matching
metadata with 140 nested unknown lists. Before the patch metadata reaches the
application's late guard; after the patch the network decoder rejects it.

The two dynamic-value tests in bencode_value.rs use an inline synthetic torrent
instead of the upstream Ubuntu fixture, which is absent from the crate archive.
This fixes their original missing-file failures without downloading test data.
Other source files and Cargo manifests are unchanged from the crate archive.
Reassess this patch when upgrading librqbit; remove it once upstream provides an
equivalent bounded decoder. Unusually nested extension data will be rejected.
