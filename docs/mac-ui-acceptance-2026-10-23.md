# Installed Slint Dock, keyboard and Retina checks — 23 October 2026

The installed ARM64 Slint test app was checked on macOS 27.0.1 (26A434),
Apple M3 Pro. Keyboard actions and the current Retina layout have bounded
positive evidence; **Cmd+O fails with the Russian input source** and remains
an open keyboard acceptance issue.

## Application and controlled state

This used `~/Applications/FlowTorrent.app`, version 0.1.0, ad-hoc package source
`2dba9f6`, with executable SHA-256
`c70c3f504e49471ec7c9d5dce31f76cba919b19cf3c7d05b7473f6022fb59975`.
Application source, UI and locked dependencies still match that package.
No application rebuild or code change was made for these checks.

The initial list was empty. A private three-file synthetic torrent, with long
Russian/English names and an unavailable loopback tracker, exercised selection,
Waiting for sources, pause/resume and removal controls. Only file index 2 was
selected; the saved manifest confirmed that selection and the separate test
destination. No external swarm or completed download was needed for this UI
check; completion and system notifications were tested in
[the preceding checks](mac-notifications-2026-10-23.md).

## Keyboard results

| Action | Observed result |
| --- | --- |
| Settings | Tab reached Settings; Return opened it and focused Change. Tab/Space toggled the notification checkbox and restored it. Tab/Return expanded Speed. |
| Invalid settings | Save activated by Return rejected a nonnumeric rate with the minimum-rate explanation. Escape canceled the panel; `settings.json` stayed absent, as initially. Valid edited settings were not saved during this check. |
| Native pickers | Tab/Return invoked Add torrent and Change. Cmd+Shift+G invoked native Go To; Return confirmed the selected torrent and folder. Path text was assigned through accessibility because automated paste failed, so this is not claimed as a complete keyboard-only native-picker path. |
| File selection | Tab reached the file expander and each checkbox. Return expanded the list; Space cleared all three choices and disabled Download. Space selected only the third; Tab/Return confirmed Download. |
| List actions | Tab reached the row and Return selected it. Space paused; Return resumed; Space paused again. The saved desired state was paused. Tab/Return on Open folder opened the expected folder in Finder. |
| Remove confirmation | Tab/Return opened Remove from list. Escape canceled it, leaving the record intact. Final removal is recorded under restoration below. |
| Quit / paused restore | After the manual Dock checks, a fresh Tab/Space pause was verified in the manifest. Cmd+Q exited PID 8350; restarting as PID 8393 restored the row as Paused with the same file selection. |
| Magnet panel | Tab/Return opened the panel and focused its input. Typing `1` and Return showed validation feedback; Escape returned to the list. No valid magnet resolution was exercised. |
| Cmd+O, manual comparison | The user confirmed that the native torrent picker opens with the English input source and does not open with Russian. This is a reproduced acceptance defect, not a passing shortcut check. |

The current shortcut condition in `ui/main.slint` accepts only event text `o/O`
with the configured modifier. This is consistent with the input-source-dependent
failure. A future correction needs a regression for both input sources and must
keep the Windows shortcut behavior compatible; none was implemented in this
test-only change.

Automation also failed to paste into a native macOS Go To field, and its
Cmd+A/paste attempts inserted Cyrillic letters in a Slint field. Consequently
automated failures alone are not treated as proof of a Slint clipboard defect.
Shift+Tab reverse traversal and manual clipboard shortcuts remain unconfirmed.
The user comparison above independently establishes the Cmd+O defect.

## Retina inspection

`system_profiler SPDisplaysDataType` reported the built-in Liquid Retina XDR
display, 3024×1964 Retina, as the sole online/main display. The installed app
was inspected in the current system dark theme at its ordinary window size
and after native window zoom, then returned to its original size. Captures
were 1640×1184 and 3024×1746 pixels respectively.

The list, long Russian/English file names, selection checkboxes, folder field,
settings, speed controls and magnet error remained readable. Labels, vector
icons, borders and keyboard focus were visually clear. Long final paths used
ellipsis; the preview's scroll area could clip its earlier content while
keyboard focus scrolled to the file checkboxes. Expanded speed controls fit
the zoomed window; the ordinary window used its existing scroll area.

This is evidence for this physical Retina display and current theme. It does
not establish non-Retina behavior, monitor transitions, alternative system
scales, the minimum window size, light theme or Russian UI locale. Russian
input-source testing above is distinct from testing a localized Russian UI.

## Dock and restoration

The user manually confirmed both actions: closing the Slint window and clicking
its Dock icon returned the window; the Dock icon's context-menu Quit exited the
app. The original PID 7502 was independently confirmed absent before relaunch.
The saved record at that point was active, and relaunch restored Waiting for
sources. A separate freshly verified pause/Command-Q/restart check is recorded
above; paused intent is not inferred across the manual testing interval.

The automation interface could not access Dock itself, so menu clicking and
window return rely on this explicit user confirmation. The menu-bar status-icon
menu was not tested. No Dock preferences or persistent pinning were changed.

Finally, Tab reached Remove from list; Return opened its native confirmation,
and Return confirmed removal. The list and manifest are empty again. All three
fixture payload files retained their previous sizes and SHA-256 hashes. These
were offline placeholders, not completed payloads. The normal position counter
advanced from 4 to 5. `settings.json` remains absent, and the Slint app is left
open in PID 8393 at its original window size.

Raw observations and screenshots are local under
`/Users/yevgeniygolota/Documents/Projects/FlowTorrent/target/mac-ui-acceptance-2026-10-23/`.
They are ignored by Git and have not been uploaded. The original checkout's
preexisting changes were preserved. The AppKit experiment remained in PID 89250
with executable SHA-256
`68d8a0c9aeb05b5f0659e4bc0159bff6dc1b5df0d8aa8795e242e7ff0798d74a`.
The installed Slint executable also remained unchanged. No Windows UI, app code,
dependency or workflow changed; no release, tag or installer upload was made.

The Russian-input Cmd+O defect, strict native-picker keyboard entry, clipboard
and reverse traversal, other themes/locales/displays and physical Windows
acceptance remain open in [the release checklist](release-checklist.md).
