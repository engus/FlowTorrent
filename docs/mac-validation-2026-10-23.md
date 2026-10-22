# Installed Slint validation on Mac — 23 October 2026

Installation, native file opening, background transfer and two real sleep/wake
cycles passed on this Apple Silicon Mac, running macOS 27.0.1 (26A434).
These are bounded local checks. They do not make 1.0 release-ready.

## Package and installation

The ARM64 test DMG was mounted read-only and copied to the previously absent
`~/Applications/FlowTorrent.app`. Its installed executable matched the mounted
copy and passed strict ad-hoc signature verification. Finder launched that
installed copy; the mounted image was detached.

- Version: 0.1.0, local test build; package source `2dba9f6`.
- DMG SHA-256: `86835e29c47d7f6a2b1e3b45c7143f00df7ea6448bf383f255f5f588db59c385`.
- Installed executable SHA-256: `c70c3f504e49471ec7c9d5dce31f76cba919b19cf3c7d05b7473f6022fb59975`.
- Rust application source, Slint UI, Cargo dependencies and vendor files match
  the code that subsequently passed [native CI](windows-validation-2026-10-23.md).

This tests installation into this existing account. It does not establish
clean-system Gatekeeper acceptance, Developer ID signing/notarization, macOS 14
runtime behavior or physical Intel compatibility.

## Observed behavior

| Check | Actual result |
| --- | --- |
| Cold file opening | Finder Open With delivered `проверка фон.torrent` to the installed app's preview. Cancel left no payload in Downloads or the test destination. |
| Warm file opening after closing the window | A different torrent reached its preview in the same process, PID 6122. Cancel left no payload. Transmission remained the default in Finder's Open With menu. |
| Add torrent / destination picker | The native file picker selected the fixture; the native folder picker selected the separate test destination. Download began only after confirmation. |
| Background transfer | After closing the window, correct payload bytes increased from 5,292,032 to 10,780,672 over 43.23 seconds; PID 6122 remained alive. Reopening the app in Finder returned the download window. |
| Pause | Correct bytes stayed at 14,548,992 for 43.46 seconds; the manifest recorded `paused`. |
| Paused sleep/wake | The system power log recorded Clamshell Sleep at 00:51:35 and wake at 00:51:43, eight seconds. The same PID, paused intent and 14,548,992 correct bytes survived. |
| Active sleep/wake in the background | `pmset sleepnow` produced Software Sleep at 00:52:36 and manual wake at 00:53:02, 26 seconds. PID 6122 and active intent survived; correct bytes grew from 18,186,240 before sleep to 18,694,144 in the first sample after wake, then to completion without Resume. |
| Complete payload | Seeding appeared. All 33,554,432 bytes matched the source SHA-256 below. |
| Quit and restore | Pause changed Seeding to Completed; Command-Q exited PID 6122. After the original `.torrent` was renamed out of the way, Finder launch restored Completed/paused from saved metadata in PID 6470. |
| Forget preserves payload | Remove from list and its confirmation emptied the list; the downloaded file remained with the same SHA-256. |

Sleep/wake timestamps above use Asia/Almaty (UTC+05:00). System `pmset` records
and the independent observer's wall-clock gaps distinguish actual system sleep
from display sleep or locking the screen.

## Controlled fixture and evidence

The 32 MiB synthetic torrent was private, advertised only a loopback HTTP tracker
and downloaded from a loopback peer limited to 131,072 bytes/second. Its tracker,
handshake, bitfield and piece bytes passed a fixture preflight. The source and
client ran on the same Mac and both suspended during sleep. This does not test
a remote peer, Wi-Fi reconnection, long sleep, repeated overnight cycles or
network changes while asleep.

Source and downloaded-file SHA-256:
`a4780d3c84a1b24b7ec96a6c5abc8b5d7405139ba0b9e553eb3d3e3e7c63a8a0`.

Local raw evidence, screenshots, fixture and retained download are under:
`/Users/yevgeniygolota/Documents/Projects/FlowTorrent/target/mac-acceptance-2026-10-23/`.
The `evidence/` folder contains install/baseline/background/pause/sleep-wake/
completion/final JSON, system power records, observer samples and screenshots.
These local fixture files are ignored by Git and have not been uploaded.

The test list was returned to empty, the original fixture filename restored and
both helper processes stopped. The installed Slint app is left open. Preexisting
state files were preserved; no existing application was replaced. The AppKit
experiment stayed in its original worktree and PID 89250; its executable hash
remained `68d8a0c9aeb05b5f0659e4bc0159bff6dc1b5df0d8aa8795e242e7ff0798d74a`.

No application code or Windows UI changed. No release, tag or installer upload
was performed. A subsequent [notification check](mac-notifications-2026-10-23.md)
passed native delivery, system denial and suppression of repeats on this Mac.
Physical Windows, supported-version Mac acceptance, remote-network recovery,
overnight operation and signing remain
open in [the release checklist](release-checklist.md).
