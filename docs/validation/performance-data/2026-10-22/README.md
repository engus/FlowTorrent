# Release resource evidence — 22 October 2026

See [the protocol and results](../../performance.md) for workload, units and limitations. The current five-scenario datasets were collected at source `0c990baa34b7f1982affed38a88f491da35bfcc6`, with clean working directories and release builds. Local samples are 30 seconds; CI samples are 15 seconds, each following a 20-second warmup. CI provenance: [Release validation 39683655107](https://github.com/engus/FlowTorrent/actions/runs/39683655107).

- `windows-local`, `windows-ci`, `macos-arm-ci`, `macos-intel-ci`: each scenario contains original `samples.csv` and `environment.json`. Per-dataset `summary.json` derives sample count, elapsed interval, last-minus-first verified progress, CPU-time delta divided by elapsed time, median and peak memory from that CSV. Unsupported metrics are null in JSON and empty in CSV.
- Each CI dataset also contains `scale125-empty`, ten samples at an explicit Slint scale of 1.25, kept separate from the default-scale summary.
- `windows-cycles-300s` preserves an earlier five-minute run and `analysis.json`. The original scale field was read before event-loop initialization and is not authoritative. Its analysis compares the first and last 15 **visible** samples. This run is not overnight evidence.

The committed files contain synthetic fixture names and numeric counters. Runtime logs, temporary payloads and seeder data are not part of these datasets. `revision_in_working_directory` describes the checkout used by the probe; it is not an embedded binary attestation. Reproduction commands are in the protocol; use a fresh output directory for every run.
