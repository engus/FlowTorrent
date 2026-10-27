# Resource measurements

Measure release builds with the software renderer. The earlier 50–150 MB range is a planning estimate, not a limit or a measured result. Do not compare Windows private bytes with macOS resident size as if they were the same quantity.

## Reproducible probe

`examples/measure_resources.rs` runs a real controller, Slint UI and background integration in a dedicated child process, with isolated application state. Active workloads use a separate loopback seeder process. Only the client PID is sampled; the orchestrator and seeder are excluded. It does not touch the normal application's configuration or public trackers.

```powershell
cargo build --locked --release --example measure_resources -j 2
target/release/examples/measure_resources.exe C:\absolute\new-probe-dir empty 60
```

On Mac use `target/release/examples/measure_resources /absolute/new-probe-dir empty 60`. The destination must not already exist. Repeat with a different destination for each scenario:

| Scenario | Workload |
| --- | --- |
| `empty` | Empty visible application window |
| `active5` | Five active transfers, visible window |
| `paused20` | Twenty real paused records, visible window |
| `hidden5` | Same five active transfers; window renders for two seconds, then hides before the sampling warmup ends |
| `cycles5` | Same five transfers, show/hide every five seconds |

Five active files are 64 MiB each. One shared download limit of 655,360 B/s keeps them active during a short run; the client has a two-worker Tokio runtime. Twenty paused files are 128 KiB each. All torrents have distinct infohashes. Network discovery is disabled in this reproducible loopback experiment, so it is not a measurement of DHT/tracker overhead on a public swarm.

The harness uses the real UI/controller and native background shell, but omits the single-instance IPC service. Report its numbers as this controlled workload, not a universal memory bound for the installed application.

The probe waits for initialization and UI readiness, then allows 20 seconds to settle before sampling once per second. `samples.csv` records elapsed time, client resident bytes (Windows working set), Windows private bytes, macOS physical footprint, cumulative CPU time, checked payload bytes and visible/hidden state. Unsupported metrics are empty, never zero. `environment.json` records scenario, OS/version, architecture, profile, renderer, PIDs, source revision/dirty status and workload. Keep both files together.

Scale reporting waits until the UI event loop has initialized. The earlier probe recorded the initial value 1 before the real scale override was applied; those old display-scale fields are not authoritative. The release workflow additionally runs at an explicit scale of 1.25 and verifies its reported value. Slint scale overrides test layout; they do not replace native dialogs and per-monitor DPI tests after changing actual OS settings.

The client writes one small status file per second for workload labels; this instrumentation adds a little IO/CPU overhead. The normal application does not write that file. Windows metrics use [PROCESS_MEMORY_COUNTERS_EX](https://learn.microsoft.com/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex). Mac uses `proc_pid_rusage`: [XNU's collection](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/kern/bsd_kern.c) reports resident/physical footprint and Mach CPU time, converted with the machine timebase ([task accounting](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/kern/task.c)). A unit check covers the Apple Silicon 125/3 conversion; treating these ticks as nanoseconds would understate CPU usage.

For each metric report median and peak over the sample window, duration and actual transfer progress. CPU usage is the change in process CPU time divided by elapsed wall time; 100% denotes one logical core. Describe workload changes, completed downloads and checking phases instead of silently pooling them with steady transfer. The fixture keeps all its files; cleanup is an explicit operator action after checking the absolute directory.

## Long runs and comparison

- Run `cycles5` for repeated show/hide, comparing the same visible phases after warmup. A single lower hidden sample does not prove absence of a leak.
- Run the application overnight with five active/seeding jobs and twenty records, capture periodic samples and verify payload hashes afterwards. The short bounded harness alone does not satisfy the overnight gate.
- Test sleep/wake on real Windows and Mac with an active transfer. A TCP relay disconnect proves reconnect behavior, not OS suspend behavior.
- Repeat on Windows 10/11, Mac ARM and Mac Intel, recording OS version and display scale. CI runners provide automation evidence but do not replace physical-display measurements.

## Results

Source revision [`0c990ba`](https://github.com/engus/FlowTorrent/commit/0c990baa34b7f1982affed38a88f491da35bfcc6), clean working directory, release profile, software renderer. [Release validation 39683655107](https://github.com/engus/FlowTorrent/actions/runs/39683655107) passed Windows x64, Mac ARM and Mac Intel. The reports below preserve raw [CSV and environment records](performance-data/2026-10-22/README.md); they measure the controlled workloads described above, with public discovery disabled and single-instance IPC omitted.

| Dataset | OS/build | Architecture | Actual Slint scale | Samples per scenario / warmup |
| --- | --- | --- | ---: | --- |
| [Windows local](performance-data/2026-10-22/windows-local/summary.json) | Windows 10.0.26300.9711 | x86_64 | 2 | 30 seconds / 20 seconds |
| [Windows CI](performance-data/2026-10-22/windows-ci/summary.json) | Windows Server runner 10.0.20348.5622 | x86_64 | 1 | 15 seconds / 20 seconds |
| [Mac ARM CI](performance-data/2026-10-22/macos-arm-ci/summary.json) | macOS 15.7.9 / 24G830 | aarch64 | 1 | 15 seconds / 20 seconds |
| [Mac Intel CI](performance-data/2026-10-22/macos-intel-ci/summary.json) | macOS 15.7.9 / 24G830 | x86_64 | 1 | 15 seconds / 20 seconds |

The extra 10-second empty-window probe on each CI target reports and validates scale 1.25; its raw files are in each dataset's `scale125-empty` directory. It is a scale-readiness check, not pooled with the default-scale resource results. CI runners do not substitute for consumer Windows 10/11 and physical Mac displays.

Memory values below are **median / peak MiB** (1 MiB = 1,048,576 bytes). CPU is average process usage over the sampled interval, where 100% is one logical core. A dash means the platform does not provide that metric through this sampler. Very short CI intervals can round low CPU usage to 0.00%; they do not prove zero work. Windows working set, private commitment, Mac resident pages and physical footprint have different meanings; do not rank the platforms using unlike columns.

| Dataset | Scenario | Resident / working set MiB | Private MiB | Physical footprint MiB | CPU % one core | Verified progress delta MiB |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| windows-local | `empty` | 34.06 / 34.06 | 11.73 / 11.73 | — | 2.10 | 0.00 |
| windows-local | `active5` | 39.11 / 39.11 | 14.78 / 14.78 | — | 9.15 | 18.44 |
| windows-local | `paused20` | 36.90 / 36.90 | 13.68 / 13.68 | — | 1.88 | 0.00 |
| windows-local | `hidden5` | 38.46 / 38.46 | 14.57 / 14.57 | — | 4.41 | 18.12 |
| windows-local | `cycles5` | 39.22 / 39.22 | 14.86 / 14.86 | — | 9.85 | 18.14 |
| windows-ci | `empty` | 22.80 / 22.80 | 5.94 / 5.94 | — | 0.00 | 0.00 |
| windows-ci | `active5` | 27.79 / 27.82 | 9.69 / 9.75 | — | 1.12 | 8.75 |
| windows-ci | `paused20` | 24.81 / 24.81 | 6.93 / 6.93 | — | 0.00 | 0.00 |
| windows-ci | `hidden5` | 26.52 / 26.52 | 10.68 / 10.69 | — | 0.00 | 8.77 |
| windows-ci | `cycles5` | 27.18 / 27.21 | 9.00 / 9.00 | — | 0.11 | 8.77 |
| macos-arm-ci | `empty` | 73.05 / 73.05 | — | 14.25 / 14.25 | 0.31 | 0.00 |
| macos-arm-ci | `active5` | 91.38 / 91.38 | — | 17.38 / 17.38 | 3.86 | 10.05 |
| macos-arm-ci | `paused20` | 79.34 / 79.34 | — | 15.19 / 15.19 | 0.46 | 0.00 |
| macos-arm-ci | `hidden5` | 86.52 / 86.52 | — | 16.91 / 16.91 | 1.60 | 9.38 |
| macos-arm-ci | `cycles5` | 91.44 / 91.45 | — | 17.99 / 18.00 | 2.75 | 9.94 |
| macos-intel-ci | `empty` | 47.06 / 47.06 | — | 17.07 / 17.07 | 0.27 | 0.00 |
| macos-intel-ci | `active5` | 51.43 / 51.43 | — | 19.82 / 19.84 | 4.82 | 10.06 |
| macos-intel-ci | `paused20` | 49.12 / 49.12 | — | 17.94 / 17.94 | 0.28 | 0.00 |
| macos-intel-ci | `hidden5` | 51.53 / 51.53 | — | 19.86 / 19.86 | 2.18 | 10.06 |
| macos-intel-ci | `cycles5` | 51.78 / 51.82 | — | 20.11 / 20.13 | 4.67 | 10.33 |

On the local scale-2 Windows fixture, the empty window used about 34 MiB working set / 12 MiB private commitment; five active downloads used about 39 / 15 MiB. Hiding the painted window reduced sampled CPU usage but retained most of its memory. These are bounded synthetic results, not a promise for arbitrary peer counts, large metadata or public swarms. The different scale, hardware and runner environment explain why local and CI results should be presented separately.

### Bounded repeated show/hide

A separate [300-sample Windows run](performance-data/2026-10-22/windows-cycles-300s/analysis.json) exercised 30 complete show/hide cycles with active transfers, with 150 visible samples. Comparing the first and last 15 visible samples, median private commitment changed from 15,380,480 to 15,552,512 bytes (+172,032 bytes, about 0.16 MiB); the peak was 15,560,704 bytes. Verified transfer increased by 196,067,328 bytes. This does not establish overnight stability or prove absence of every retained closure/leak.

That older run predates the scale-readiness correction. Its original environment file is preserved unchanged, including the early, non-authoritative scale value 1; do not use it for cross-scale comparisons. The current five-scenario data above uses corrected post-initialization telemetry. Overnight download/seeding, real sleep/wake and physical-display validation remain open in the [release checklist](release-checklist.md).
