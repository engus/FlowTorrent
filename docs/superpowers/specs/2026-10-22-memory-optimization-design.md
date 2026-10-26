# Memory optimization

The user requested profiling and RAM reduction, especially on Mac, while retaining
Windows compatibility. Preserve transfers, discovery, UI appearance, native
dialogs, the four settings and all storage/persistence safety guarantees.

## Findings and approach

Profile optimized arm64 builds on the current Mac at actual Slint scale 2.
Report resident size and physical footprint separately. Use the existing
loopback-only five-transfer harness for comparable runs; do not turn public
discovery into a test prerequisite.

The initial component probe measured approximately 2.3 MiB physical footprint
after engine creation, 9 MiB after UI construction, and 51 MiB after painting.
The installed app's heap contains one 9.7 MiB allocation consistent with its
software framebuffer; native graphics and allocator pages add substantial
overhead. Shared framework mappings also inflate resident memory. Engine
thread/peer limits are not the primary idle cost.

Approaches considered:

- Keep the software renderer and remove avoidable allocations. This is the
  selected engineering approach: it preserves behavior and the shared Windows
  code. Measure every claimed improvement.
- Destroy/recreate native windows on hide using Slint's existing switch.
  Experiment saved less than 1 MiB footprint and increased resident size in
  the first A/B pair. Reject this change; retain native window identity.
- Change renderer, reduce Retina resolution, or limit transfers/peers.
  Defer: these change display/throughput behavior and do not address the
  measured source without substantial new risk or evidence.

## Changes

1. Replace the full generic bencode tree used solely to detect pure-v2 metadata
   with a bounded, borrowing format probe that skips unused values. Preserve
   exact source metadata, v1/hybrid behavior, pure-v2 errors and decoder limits.
   Prove peak Rust allocation reduction with a separate-process integration
   test containing a large ignored extension list. Keep its allocator tracker
   confined to that test binary.
   Preview selection will also share the immutable preview and use one boolean
   per source file instead of cloning all paths into a separate vector and tree.
2. Experiment with Apple's public `malloc_zone_pressure_relief(NULL, 0)` after
   hiding. Ship only if repeated controlled runs demonstrate useful physical
   footprint savings. This frees unused allocator pages, never live payloads or
   UI objects. If retained, debounce and run it off the UI thread, cancel while
   visible, and keep Windows as a no-op without another worker or dependency.
3. Keep a reproducible component-attribution example and raw before/after
   reports. Rebuild the local Mac app and verify close/reopen/Quit behavior.

No dependency upgrades, replacement UI, new settings, public releases or pushes.
Windows and Intel Mac runtime savings require their own future measurements.

## Experiment decisions

The allocator experiment released zero bytes on this Mac and produced no
footprint improvement. Do not ship a reclamation hook. Native-window recreation
also failed the usefulness threshold, so all product platform code stays intact.
These decisions are supported by the saved probe output, not an assumption that
trimming is generally ineffective on every Mac workload.
