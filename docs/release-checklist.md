# FlowTorrent 1.0 release checks

This is a factual gate list, not an announcement that 1.0 is ready. Confirmed automated baseline: source `0c990ba`, [Check 39683564507](https://github.com/engus/FlowTorrent/actions/runs/39683564507) and [Release validation 39683655107](https://github.com/engus/FlowTorrent/actions/runs/39683655107), Windows x64 / Mac ARM / Mac Intel, all successful. Each Check job reports 86 passing entries, zero ignored. Release validation builds the application and runs bounded resource probes; it does not produce installers.

| Specification criterion | Evidence and remaining work | Release gate |
| --- | --- | --- |
| 1. Clean installation on supported platforms | Shared core/UI compile in the CI matrix; installers and clean Windows 10/11, macOS 14+ installation checks pending | Open |
| 2. Button / OS / warm hidden opening | Windows controlled GUI and warm IPC tested; both AppKit probes pass; real installed association and physical Finder/Dock checks pending | Open |
| 3. Single/multifile payload and selection | Real local swarms, exact selected bytes across a nonaligned piece boundary, paused restart; 256-file low-soft-FD regression passes on both Macs | Automated evidence |
| 4. Duplicates and file conflicts | Same-infohash queue, no-clobber, capability storage and link-swap tests | Automated evidence |
| 5. Pause, resume, completion and seeding | Real engine/controller tests, Windows GUI, immediate pause during checking and error retry | Automated evidence; physical Mac UI pending |
| 6. Graceful/crash restore | Forced-child Add/Pause/Forget boundaries; partial reuse; original torrent removed; Windows UI force-close/restart | Automated evidence; packaged systems pending |
| 7. Faults, sleep/wake | Missing-volume isolation/retry; storage write faults, corrupted-piece recheck and relay reconnection covered by stage 9 tests; real sleep/wake pending | Open |
| 8. Global download/upload limits | Minimum-rate validation, two-job aggregate download and two independent downloaders sharing one aggregate upload cap | Automated evidence |
| 9. Forget keeps payload; close vs Quit | Core tests and Windows hidden/reopen/graceful-Quit harness; installed native menu checks pending | Open |
| 10. Overnight/resources | [Raw release measurements](performance.md): five scenarios on all three CI targets and local Windows; bounded 300-second show/hide stress. Overnight workload remains unverified | Open |
| 11. Languages/themes/keyboard/scales | RU/light, EN/dark and labels inspected; Windows list at Slint overrides 1/1.25/1.5/2, settings at 1/1.5/2 and Tab/Enter checked. Full keyboard workflow, native per-monitor DPI/dialogs and physical Mac Retina/ordinary display pending | Open |
| 12. Packages/update/signing | Stage 10 not implemented; credentials not supplied | Open |

Installed notifications must deliver once in the background; denial must leave transfers working. Crash-between-persist-and-notify may omit a notification by the agreed at-most-once policy. Windows Start Menu shortcut and Mac bundle use `com.engus.flowtorrent`.

Before public release: complete the matrix and evidence, run release builds and exact-lockfile license collection, test installation/update/uninstall without payload deletion, sign Windows / Developer ID + notarize Mac, prepare checksums and release notes, and perform the final whole-branch review. Publishing a public GitHub Release is a separate action.
