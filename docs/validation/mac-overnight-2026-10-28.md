[Русский](mac-overnight-2026-10-28-ru.md)

# Controlled Mac overnight Slint run — 28 October 2026

## Verdict and coverage

**The controlled eight-hour transfer/endurance run passed. Eight continuous hours with the window hidden were not achieved.** The window was hidden for **1h29m50s**, then reopened around90minutes into sampling and stayed natively visible; total visible coverage including startup was6h30m51s. Do not label this an eight-hour background-window pass. Native is_visible does not prove physical exposure, focus or an awake screen.

There were **5760 five-second samples spanning8h00m41s**, with no incomplete CSV row, no torrent error, no lost paused record and no interrupted child process. Wall time was **22:30:52 on27October–06:32:13 on28October, Asia/Almaty**, including startup,20s sampling warmup and shutdown. Small per-sample overhead explains the measured40s beyond eight hours.

Five128MiB torrents (640MiB total) were completely downloaded and sent to the controlled receiver. Independent post-run full-file checks recomputed **ten SHA-1 and SHA256 matches** against the five source files, verified equal134217728-byte lengths and cross-checked the recorded payload-verification.json. Final state: **five Seeding, twenty Paused, zero errors/global error**. Client/receiver checked bytes671088640; upload counter671137792 (640MiB+48KiB). The receiver's only peer was the measured client; private metadata disabled PEX, so receiver completion and upload counters provide evidence of real client seeding.

## Source and protocol

Main142f2bb75a4c4c8769f15a44d0f4fface33406b0, production tree clean at launch. This is an ad-hoc-signed instrumented standalone .app linked to the unchanged desktop library, **not the Slint optimization candidate**. Binary SHA256 `e159d0d47ebb5e1d5b9969533accdfc4c410f2f4875bdc1d34d415bf38fef18b` and recorded harness/lock/resource-source hashes were independently rechecked. Production desktop/assets/vendor/Cargo inputs still match main HEAD. AppKit prototype unchanged. No commit/push/tag/release/installer publication or file deletion.

Apple Silicon, macOS27.2/26B123, release Rust1.92, Slint1.18.1 software renderer, native scale2. Source/client/receiver had separate PIDs; metrics sample **client10245 only**. Main global down/up caps32768B/s, receiver24576B/s. Five128MiB active fixtures plus20paused128KiB records. Loopback-only private metadata, no trackers/DHT/public discovery, no single-instance IPC, no completion notifications. JSON status writes and sampling add diagnostic overhead absent from normal production. Window size/exposure were not frozen as a controlled visual benchmark.

All three owned harness processes exited gracefully; the driver/orchestrator and owned caffeinate also exited. Their stderr logs were empty. The final completion marker, raw CSV, final per-row states, source/binary hashes and full payloads were checked separately. Original driver was not restarted; later candidate builds used separate copied apps and did not replace this binary.

## CPU and memory

CPU100%=one logical core. RSS and physical footprint are distinct and must not be summed. Each table row is a **continuous** visibility/workload/external-load segment; no CPU delta crosses an excluded gap. Workload boundaries use full client bytes, then full receiver bytes, rather than treating a brief WaitingForSources state as a new workload. Values are segment CPU rates and memory medians. Startup and externally loaded rows are retained as evidence, not described as uncontended steady-state measurements.

On the settled hidden30–90minute interval (719 samples), CPU averaged **0.66%**, median RSS **135.34MiB**, median footprint **54.83MiB**. First/last five-minute medians were RSS135.27→135.50MiB and footprint54.75→55.00MiB: small changes in this observed hour, not proof of general leak freedom. The full hidden phase includes startup/cache release, so its memory decline should not be interpreted as negative leak growth.

After reopening, uncontended visible download segments ranged roughly0.76–11.12% CPU; window exposure and screen state were uncontrolled, so these are not comparable rendering benchmarks. The long seeding-with-receiver phase averaged0.53%; the final25-minute idle-seeding phase0.39%. These low rates do not establish equivalent visible rendering throughout the night.

Final RSS was **92.50MiB**, final footprint **75.16MiB**. Whole-run peaks were234.91MiB RSS and159.78MiB footprint, during visible/download activity. Do not infer memory neutrality from a lower final number: during late receiver-active seeding footprint jumped **+23.36MiB at7h20m31s**; after receiver completion it dropped **43.88MiB at7h35m58s**. The phase's first/last five-minute footprint medians rose95.70→119.16MiB; idle medians then settled75.14→75.16MiB. A monotonically growing leak was not demonstrated, but there were substantial transient increases. No late allocation map was captured, so their owner/cause is unproven.

| Interval since sampling started | Window / workload | CPU | RSS MiB | Footprint MiB | External work |
| --- | --- | ---: | ---: | ---: | --- |
| 00:00:00–00:00:40 | visible; downloading | 4.83% | 176.89 | 98.24 | none recorded |
| 00:00:45–01:30:30 | hidden; downloading | 0.66% | 135.36 | 54.83 | none recorded |
| 01:30:35–01:33:40 | visible; downloading | 7.07% | 158.71 | 84.83 | none recorded |
| 01:33:45–01:35:05 | visible; downloading | 10.48% | 190.20 | 116.91 | cpu-investigation/profile-window.json |
| 01:35:10–01:38:30 | visible; downloading | 11.12% | 173.03 | 116.91 | none recorded |
| 01:38:35–02:12:53 | visible; downloading | 9.19% | 130.58 | 116.97 | concurrent-redraw-work.json |
| 02:12:58–02:27:39 | visible; downloading | 10.24% | 130.77 | 117.00 | none recorded |
| 02:27:44–03:36:04 | visible; downloading | 6.60% | 136.83 | 117.58 | concurrent-ram-work.json |
| 03:36:09–03:52:15 | visible; downloading | 5.28% | 134.08 | 117.66 | none recorded |
| 03:52:20–05:26:33 | visible; downloading | 4.00% | 132.95 | 117.49 | concurrent-ram-fix-work.json |
| 05:26:38–05:41:00 | visible; downloading | 0.76% | 89.39 | 95.64 | none recorded |
| 05:41:05–07:35:23 | visible; seeding with receiver | 0.53% | 89.59 | 95.84 | none recorded |
| 07:35:28–08:00:41 | visible; idle seeding | 0.39% | 92.50 | 75.16 | none recorded |

## Interference and limits

All three `concurrent-*.json` windows (redraw work, longer RAM comparison and RAM-fix work) plus the read-only profiler interval were excluded from ordinary CPU interpretation. Intervals use original CSV filesystem birthtime to align elapsed samples to UTC, with10s padding for timestamp/5s-sample uncertainty. The JSON audit records exact boundaries. External windows conservatively include dispatch/verification gaps. No single pooled CPU/RAM average is used as a leak or hidden-window result.

The optimization worktree's partial mmap fix and its inconclusive CPU comparison are separate experiments. This night does not validate either candidate. Source and copied nightly binary remained unchanged even while their build output directory was reused elsewhere.

This run proves controlled local transfer endurance and approximately90minutes with a hidden window. It does not prove public-swarm discovery/DHT/trackers, Wi-Fi recovery, OS sleep/wake, ordinary installed-app IPC/notifications, eight uninterrupted hidden hours, Windows/Intel physical acceptance or universal leak freedom. Caffeinate prevented idle/system sleep; display sleep remained allowed. No fresh physical screen-state audit was performed.

Raw evidence remains under ignored `_work/validation/overnight-20261027/overnight-20261027T173052Z/`; independent-final-verification.json contains recomputed hashes, continuous segments, source checks, timestamps and exclusions. No files were removed. Continuous-hidden and real-network/platform release gates remain open. The completed nightly monitor is removed after saving this report; no new run is started automatically.
