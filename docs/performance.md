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

No stage 9 resource results are recorded yet. Add actual probe reports here after a successful release run; do not infer them from debug tests or the older combined seeder/UI smoke example.
