# Memory profiling and optimization

22 October 2026. Release arm64 builds on macOS 27.0.1, Rust 1.92.0, software
renderer, actual Slint scale 2. Changes are on `codex/memory-optimization`.

## What uses memory

The empty app is dominated by rendering and native UI/framework memory. An
exploratory cumulative component probe measured physical footprints of about
1.9 MiB at process entry, 2.3 MiB after the controlled engine, 9 MiB after UI
construction, and 51 MiB after painting. These stages include preceding stages;
they are not independent additive measurements.

The installed app's native heap inspection found about 17.6 MiB allocated,
including one 9.7 MiB block consistent with the full-window software framebuffer.
A separate stack-logged fresh-window trace confirmed a 7,356,416-byte
software-renderer allocation and a 7,389,184-byte CoreAnimation image copy.
The installed window had already been resized during GUI checks; its larger
buffer is not the initial-window baseline. Physical footprint was about 52.5 MiB. Resident size also includes clean
code and shared framework mappings, and must not be treated as the same metric.
Virtual reservations in vmmap are not allocated RAM either.

The pinned softbuffer implementation allocates a pixel buffer for a frame and
hands ownership to a CoreGraphics image provider. That explains why retaining
the last painted window has a cost even with no transfers. See the
[software renderer documentation](https://docs.slint.dev/latest/docs/rust/slint/platform/software_renderer/struct.SoftwareRenderer)
and [upstream softbuffer source](https://github.com/rust-windowing/softbuffer/blob/v0.4.8/src/backends/cg.rs).

## Changes made

Two portable changes avoid extra allocations when opening torrent metadata:

| Controlled allocation regression | Before | After |
| --- | ---: | ---: |
| Inspect a 900,741-byte v1 torrent with a large ignored extension list | 30,337,050 bytes | 1,500,384 bytes |
| Set up selection for 10,000 files with long paths, including padding | 2,885,536 bytes | 10,000 bytes |

These are additional live Rust allocation peaks in the debug-profile isolated test binary,
not total application RAM or OS physical footprint. The stress cases expose
avoidable amplification; ordinary small torrents have smaller savings.

Format detection now reads only format markers and skips other values, instead
of constructing a full generic bencode tree before the real engine decoder.
The original bytes and infohash remain unchanged. Pure-v2/hybrid classification,
noninteger extension markers, nesting and metadata size limits remain covered.

Preview selection shares immutable preview data with `Arc` and stores one
boolean per source file instead of copying filenames and building a tree of
selected IDs. Padding cannot be selected, sparse IDs remain valid, returned
IDs remain sorted, and dropping the selection releases its shared owner.

Both changes use the same Rust code on Windows and Mac. No library versions,
network/discovery defaults, display quality, settings or storage policy changed.

## Experiments rejected

- Slint's native-window destruction switch: hidden workload footprint changed
  from 52.36 to 51.86 MiB in the first pair, while resident size increased.
  That is not useful evidence for a product change, and native-window identity
  would need additional lifecycle validation. The switch is not enabled.
- Apple's `malloc_zone_pressure_relief(NULL, 0)`: returned zero released bytes
  in the component experiment, with no useful footprint reduction. No trimming
  hook runs in the app. `memory_breakdown --relief` retains it only as an opt-in
  diagnostic. [Apple's API contract](https://github.com/apple-oss-distributions/libmalloc/blob/main/include/malloc/malloc.h)
  describes reclamation of unused allocations, not release of live objects.

## Release workload comparison

Physical footprint on this Mac (not resident size):

| Scenario | Before median / peak MiB | After median / peak MiB | Before / after progress MiB |
| --- | ---: | ---: | ---: |
| empty | 52.08 / 52.14 | 35.47 / 35.67 | 0.00 / 0.00 |
| active5 | 51.58 / 52.84 | 51.46 / 52.00 | 11.89 / 11.89 |
| paused20 | 34.89 / 35.13 | 44.80 / 52.05 | 0.00 / 0.00 |
| hidden5 | 52.36 / 52.41 | 45.11 / 52.58 | 12.14 / 11.88 |
| cycles5 | 50.69 / 50.80 | 52.31 / 52.42 | 11.88 / 11.88 |

These short runs show **no consistent steady-state improvement**: paused20 and
cycles5 increased while other scenarios decreased. An additional run of the
unchanged baseline measured 50.16 MiB empty, versus its first 52.08 MiB. Native
rendering/caching and short-run variation prevent attributing the idle difference
to code paths that do not run in the empty app. Only the isolated metadata and
selection allocation reductions above are claimed as reproduced improvements.
All active workloads advanced checked payload bytes; no throughput limit changed.
CPU, resident metrics, exact environment, source IDs, executable hashes and raw
CSV are preserved in the [measurement dataset](performance-data/2026-10-22-memory/mac-arm-local/README.md).

The same existing harness uses an isolated loopback seeder, five active
64 MiB transfers, twenty paused records, hidden transfers and repeated hide/show.
Each scenario has 20 seconds of warmup and 20 one-second samples. Client-only
measurements exclude the seeder; public discovery and IPC are omitted. These
short runs are not an overnight or arbitrary public-swarm guarantee.

## What can reduce memory further

Avoid duplicating metadata and file tables; these changes address measured
instances. A future lazy preview-row model could format only displayed rows
instead of storing all UI strings. For very large public swarms, first measure
peer and block queues before considering bounds that could affect throughput.

For the Mac idle baseline, graphics buffers and native caches need a separate
renderer experiment before promising further savings. Pixel-buffer cost grows
with physical window area; reducing display resolution would trade away visual
quality, so it was not used as an optimization. Rewriting the Mac UI separately
would also sacrifice the current shared UI architecture. Neither is required
for the improvements delivered here.

No dramatic idle-memory reduction is claimed. Windows/Intel Mac savings and
native behavior still need fresh measurements on those platforms.

## Reproduction and verification

Use the pinned toolchain and `MACOSX_DEPLOYMENT_TARGET=14.0` on Mac:

```sh
cargo test --locked --test metadata_memory -j 2 -- --nocapture
cargo build --locked --release --example measure_resources --example memory_breakdown -j 2
target/release/examples/memory_breakdown /absolute/new-component-fixture
target/release/examples/measure_resources /absolute/new-workload-fixture active5 20
```

The Mac packaging command applies the documented host proc-macro strip
workaround on this macOS version. See [performance.md](performance.md) for
the workload protocol and [the implementation plan](superpowers/plans/2026-10-22-memory-optimization.md).

Validation: fmt and all-target Clippy pass; 112 all-target Rust tests, 18 decoder
tests and 9 Python tests pass. The native Mac lifecycle probe and the existing
dependency policy scan pass. One independent source review found no issues
requiring changes. The local arm64 `.app` was rebuilt with deployment target 14.0 and strict ad hoc
signature verification. Its executable code/data sections match the release
output; signed executable hashes are recorded in the dataset. No remote
publication occurred.

GUI smoke in the installed app: opened a controlled 10,000-file torrent preview,
expanded its virtualized list, deselected the large first file and observed the
selected total change from 524 MB to 10.0 KB. Cancel returned to an empty registry;
the payload directory was not created. Window close/reopen actions were exercised,
Cmd+Q process exit was confirmed, and the app was relaunched and left ready.
Hidden transfer progress and hide/show cycles passed in the controlled probes.
Windows and Intel Mac CI/physical validation were not run for this branch.
