# Local Mac memory measurements

22 October 2026, macOS 27.0.1 / arm64, Rust 1.92.0, software renderer, actual scale 2.

- `before/`: release probe built from `0cb7184` before product edits. A preserved
  executable copy was used for later baseline scenarios and the empty control.
- `after/`: release probe built from `ccea1af`, including `a2f5906` optimizations.
- `summary.json`: client-only median/peak resident and physical footprint, CPU
  (% one logical core) and verified transfer-byte deltas.20 samples after20s warmup.
- `binaries.json`: exact SHA-256 of each probe executable; binaries are not committed.
- `components-*.txt`: cumulative exploratory component attribution. Final example
  reports actual scale 2 and 1640×1120 physical window geometry after painting.
- `experiments/`: rejected window-destruction experiment and unchanged-baseline
  empty control, each with unmodified CSV/environment metadata.
- `reclamation-experiment.txt`: opt-in public allocator API returned zero bytes.

Original CSV/environment files are copied unchanged. Before metadata may record
an exploratory example as untracked: the preserved benchmark executable predates
those edits. After metadata may record new report files: product source is `ccea1af`.
The probe omits public discovery and IPC; it does not modify the real app state.
Each active file is 64 MiB, with one shared 655,360 B/s client limit. Seeder memory
is excluded. No native allocation stack logging was enabled for comparison runs.

See [interpretation and limits](../../../../architecture/memory-optimization.md). Ordinary workload
results vary and are not presented as a proven overall RAM reduction. The isolated
allocation regression demonstrates the claimed metadata/selection improvements.
