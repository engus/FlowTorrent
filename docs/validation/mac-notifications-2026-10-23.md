# Installed Slint notifications on Mac — 23 October 2026

Completion delivery, system permission denial and suppression of repeat
notifications passed on this Apple Silicon Mac, macOS 27.0.1 (26A434).
This extends [the installed-app checks](mac-validation-2026-10-23.md);
it does not close the Windows or supported-version Mac release gates.

## Application and fixture

The actual installed `~/Applications/FlowTorrent.app` was used, version 0.1.0,
local ad-hoc test package from source `2dba9f6`. Its executable SHA-256 remained
`c70c3f504e49471ec7c9d5dce31f76cba919b19cf3c7d05b7473f6022fb59975`.
There was no rebuild or application code change.

Three private synthetic torrents used only a loopback tracker and peer, limited
to 131,072 bytes/second. Each contained 2,097,152 bytes and completed with SHA-256
`18733abf4f01ac8338af027adec47bb194c3c2170cdc30a78a252a81790c1de5`.
The app's completion-notification setting retained its enabled default;
`settings.json` was absent before and after testing.

## Observed results

| Check | Evidence and outcome |
| --- | --- |
| System permission denied | With FlowTorrent notifications off in System Settings and its window closed, `notif-denied.bin` completed and reached Seeding. The app logged `Completion notification permission denied or unavailable`; its durable completion flag was true. No native notification request was accepted or delivered for this infohash. Transfers were unaffected. |
| System permission enabled | After temporarily enabling permission, `notif-allowed.bin` completed. At 01:04:18, `usernoted` accepted one request, delivered it to the configured destinations and recorded banner presentation. The user manually confirmed exactly one “Download complete” card for this file in Notification Center. Window visibility at this first enabled completion is not claimed. |
| Completion with window closed | A second enabled download, fixture `notif-denied-again.bin`, was added and the Slint window closed before completion. The persisted completion flag was still false at 01:11:21.816; no app-window observation reopened it during the transfer. At 01:11:31, macOS accepted one request, delivered it and recorded banner presentation. The payload matched the source and the completion flag became true. Despite its fixture name, this was an **allowed** background test. |
| Pause/resume and restart | The first allowed completed torrent was paused and resumed. Command-Q then exited PID 6470; launching the installed app restored all three records as Seeding in PID 7502. Native request/delivery counts stayed unchanged: denied 0/0, first allowed 1/1, background allowed 1/1. The previously denied completion was not replayed after permission was enabled or the app restarted. |

Times use Asia/Almaty (UTC+05:00). Counts compare `usernoted` pipeline-acceptance
and delivery records for each exact torrent request ID, before and after restart;
they do not count every repeated log reference as a new notification.
The native warnings contained only the expected denied-permission warning.

The first card's visual confirmation came from the user: this macOS version's
automation capture exposed the Batteries widget rather than the notification
drawer. The second background delivery has native system-log evidence; it was
not separately confirmed by the user. Global Focus settings were not changed.
Denial coverage used an existing system-level off setting, not a fresh-account
first-run permission dialog.

## Restoration and remaining scope

FlowTorrent's original system notification permission was restored to **off**
and verified in System Settings. Destination checkboxes, Temporary alert style,
preview preference and grouping were retained. Remove from list and its native
confirmations returned the app to its initially empty list, preserving all three
payload hashes. The normal position counter advanced from 1 to 4. The local
seeder exited successfully and the installed app was left open with an empty list.

Raw fixture, filtered FlowTorrent-only system logs, JSON evidence and screenshots
remain locally under
`/Users/yevgeniygolota/Documents/Projects/FlowTorrent/target/mac-notifications-2026-10-23/`.
They are ignored by Git and were not uploaded.

AppKit remained isolated in its existing worktree and PID 89250. Its executable
SHA-256 remained
`68d8a0c9aeb05b5f0659e4bc0159bff6dc1b5df0d8aa8795e242e7ff0798d74a`.
The original checkout's preexisting documentation changes were preserved.
No application source, Windows UI, dependency or workflow changed. No release,
tag or installer upload was performed.

Physical Windows notification acceptance, supported older Mac versions, fresh
permission prompts and broader release checks remain open in
[the release checklist](release-checklist.md). The agreed persist-before-notify
policy still permits omission if the process crashes between those two actions.
