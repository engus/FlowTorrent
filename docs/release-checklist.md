# FlowTorrent 1.0 release checks

This is a factual gate list, not an announcement that 1.0 is ready. Confirmed automated baseline: source `0c990ba`, [Check 39683564507](https://github.com/engus/FlowTorrent/actions/runs/39683564507) and [Release validation 39683655107](https://github.com/engus/FlowTorrent/actions/runs/39683655107), Windows x64 / Mac ARM / Mac Intel, all successful. Each Check job reports 86 passing entries, zero ignored. Release validation builds the application and runs bounded resource probes; it does not produce installers.

Local continuation on 22 October: production base `91872e0` includes the audit
fixes and memory changes, without native UI experiments. Its current ARM64 Mac
baseline is 112 passing Rust records, zero failures/ignored. Test-package scripts,
notices, Mac DMGs and a Windows per-user installer implementation are recorded
in [the local packaging report](release-readiness-2026-10-22.md). Both Mac DMGs
passed mounted-bundle verification; the ARM64 package passed a bounded GUI smoke.
See [test packaging](test-packages.md) for commands.
Fresh three-platform core CI and native packaging evidence are recorded in
[the native CI report](windows-validation-2026-10-23.md). Windows passed the
disposable per-user install/reinstall/uninstall preservation test. This does
not replace physical Windows 10/11 or macOS acceptance.

[Local installed-Mac validation on 23 October](mac-validation-2026-10-23.md)
passed Finder cold/warm file opening, background transfer, pause, two real
sleep/wake cycles (paused eight seconds, active 26 seconds), exact completed
payload verification, Command-Q/paused restore and Forget preserving payload.
It used this macOS 27 Apple Silicon account and a same-host loopback source;
supported-version clean installs, remote-network recovery and overnight tests
remain open.

| Specification criterion | Evidence and remaining work | Release gate |
| --- | --- | --- |
| 1. Clean installation on supported platforms | Native packages build and disposable Windows preservation checks pass. ARM Slint DMG installed and launched in the existing macOS 27 account; clean supported-system acceptance remains pending | Open |
| 2. Button / OS / warm hidden opening | Installed ARM Slint app passed native picker and Finder Open With cold/warm delivery with unchanged default handler. Windows controlled GUI/IPC and Mac probes pass; physical Dock and Windows association checks remain pending | Open |
| 3. Single/multifile payload and selection | Real local swarms, exact selected bytes across a nonaligned piece boundary, paused restart; 256-file low-soft-FD regression passes on both Macs | Automated evidence |
| 4. Duplicates and file conflicts | Same-infohash queue, no-clobber, capability storage and link-swap tests | Automated evidence |
| 5. Pause, resume, completion and seeding | Real engine/controller tests and Windows GUI; installed ARM Mac UI passed pause/resume, exact completion and Seeding | Automated and bounded Mac evidence |
| 6. Graceful/crash restore | Core crash tests and Windows UI; installed ARM Mac passed Command-Q and Completed/paused restore without the original torrent. Physical crash/partial restore remains pending | Partial physical evidence |
| 7. Faults, sleep/wake | Automated faults/reconnection plus installed ARM Mac paused/active sleep cycles (8/26 seconds) pass. Windows, remote-network changes and longer cycles remain unverified | Open |
| 8. Global download/upload limits | Minimum-rate validation, two-job aggregate download and two independent downloaders sharing one aggregate upload cap | Automated evidence |
| 9. Forget keeps payload; close vs Quit | Installed ARM Mac passed close/background transfer, Finder reopen, Command-Q exit and Forget with unchanged payload hash. Physical Dock/tray menu checks remain pending | Open |
| 10. Overnight/resources | [Raw release measurements](performance.md): five scenarios on all three CI targets and local Windows; bounded 300-second show/hide stress. Overnight workload remains unverified | Open |
| 11. Languages/themes/keyboard/scales | RU/light, EN/dark and labels inspected; Windows list at Slint overrides 1/1.25/1.5/2, settings at 1/1.5/2 and Tab/Enter checked. Full keyboard workflow, native per-monitor DPI/dialogs and physical Mac Retina/ordinary display pending | Open |
| 12. Packages/update/signing | Native test packages and same-version Windows reinstall/uninstall preservation checks pass. Physical installed-app validation, cross-version session upgrade, signing/notarization and final notice review pending; credentials not supplied | Open |

Installed notifications must deliver once in the background; denial must leave transfers working. Crash-between-persist-and-notify may omit a notification by the agreed at-most-once policy. Windows Start Menu shortcut and Mac bundle use `com.engus.flowtorrent`.

Before public release: complete the matrix and evidence, run release builds and exact-lockfile license collection, test installation/update/uninstall without payload deletion, sign Windows / Developer ID + notarize Mac, prepare checksums and release notes, and perform the final whole-branch review. Publishing a public GitHub Release is a separate action.
