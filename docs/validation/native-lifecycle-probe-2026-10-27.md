[Русская версия](native-lifecycle-probe-2026-10-27-ru.md)

# Native Slint lifecycle probe timing — 27 October 2026

The Mac Intel job in [the earlier PR run](https://github.com/engus/FlowTorrent/actions/runs/40497144406) passed its build, tests and Clippy but failed with `native modal lifecycle did not run`. The probe asserted completion of asynchronous native modal setup at a fixed 1.5-second timer measured from event-loop startup, making that assertion depend on runner startup and window-creation latency.

The probe now starts its 1.5-second hidden-window observation only after modal attach/detach has completed. An independent 30-second watchdog still fails an unfinished modal lifecycle or a hidden loop that does not exit. Native modal attachment, ordered delegate callbacks, reopen behavior, tray lifetime and the prohibition on late tray recreation during exit remain asserted. This changes the Slint diagnostic example only; the separate AppKit prototype and production native hooks are unchanged.

The revised probe, strict Clippy, 164 application tests, 18 bencode tests and doc-tests passed locally on the ARM Mac before committing. The earlier remote failure is evidence for the old probe, not a passing result for this revision. Fresh three-platform CI is authorized with installer uploads disabled; its actual result is recorded by GitHub Actions at the tested revision. No release publication is authorized.
