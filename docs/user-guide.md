[Русский](user-guide-ru.md) · [Documentation](README.md)

# User guide

This guide describes the current 0.1.0 development interface. Test builds are unsigned on Windows and ad-hoc signed on Mac. The supported source targets are Windows 10 22H2 / 11 x64 and macOS 14+ on Apple Silicon / Intel; remaining platform acceptance is tracked in the [release checklist](release-checklist.md).

## Install or launch a test build

On Windows, use the supplied per-user installer. On Mac, choose the build for your processor, open its DMG, then drag FlowTorrent.app to Applications. A DMG contains the app; opening the DMG alone does not install it. For source builds, see the [project README](../README.md) and [test packaging guide](test-packages.md). No public 1.0 download is announced here.

## Add and download

1. Choose **Add torrent** for a local `.torrent`, or the adjacent magnet control to paste a link. Magnets need a BitTorrent v1 hash; hybrid links with a v1 hash are supported. Pure v2 is unsupported. Paste magnets into the app; system-wide magnet registration is not included.
2. For a magnet, wait for metadata. You can cancel the lookup before any download is confirmed.
3. Review the files, select what you need and choose the destination. Confirm to begin. Existing names are not silently overwritten; a conflict requires another destination. The default folder affects new adds only.

Use **Stop** to pause and **Start** to resume. After completion, an active torrent keeps seeding until stopped, removed or the app exits. Restored torrents first check saved data, then resume if they were active; manually stopped entries stay stopped. The app retains its own metadata, so the original `.torrent` is not needed after adding.

## Find torrents and inspect files

The sidebar provides **All**, **Downloading** and **Completed** views. Downloading groups unfinished entries, including stopped or waiting ones; it is not the same as the tray's actively downloading count. Search matches names locally. Sorting changes list order. **Start all / Stop all always affect the whole registry**, including rows hidden by a filter.

Select a row, then use its ellipsis to open or toggle the bottom details drawer. Ordinary selection does not open a closed drawer. Search and modifier selection expose multi-selection controls. Drawer actions apply to the named selection. Choose one torrent to inspect its real file progress, included/excluded files and file search; multiple selection does not fabricate a combined file list.

Drag the sidebar separator to resize it, or collapse it to its icons. Resize the drawer using its separator. Layout preferences are saved when a resizing gesture finishes. The file list and information column scroll independently while their actions stay available.

Progress and speeds come from the engine. An unknown ETA is shown explicitly. **Uploaded percentage is per engine session**, so it may reset after restart; it is not a durable lifetime ratio. There is no ratio-based automatic stop.

## Settings

Changes save automatically; the pane stays open. While saving, conflicting controls are unavailable. A failed change restores the durable settings and shows an error.

- **Appearance:** System, Light or Dark.
- **Language:** System or one of 14 language autonyms. Unsupported system languages use English. See [localization](localization.md).
- **Launch at login:** changes only FlowTorrent's dedicated OS entry after you explicitly toggle it. Opening settings does not enable it.
- **Completion notifications:** optional, at most once for a completion. OS denial or delivery failure does not stop transfers.
- **Folder for new downloads:** choose a folder; existing payload stays in its current location.
- **Global download and upload limits:** Unlimited by default; presets begin at 0.5 Mbit/s. Limits apply across torrents, including upload during download and seeding. Mbit/s uses decimal bits: 0.5 Mbit/s is 62,500 bytes/s. Older saved non-preset values appear as a read-only Current value option and remain exact when another setting changes. There is no custom-rate input in the current UI.

## Background operation and exit

Closing the window keeps transfers running. On Windows, reopen from the tray; on Mac, use the Dock or menu bar icon. Use **Quit**, or **Cmd+Q** on Mac, to save state and exit the process.

The branded tray/menu bar icon stays the same. Its menu and tooltip show **↓ download**, **↑ upload** and the count of entries actually in **Downloading** state, using existing engine snapshots while the window is hidden too. Waiting, checking, paused and seeding entries are not counted as downloading. Speeds are totals, not a network benchmark.

## Remove safely or recover from errors

**Remove from list** removes the saved entry and leaves downloaded files in place. **Remove torrent and files** is a separate confirmed action that moves manifest-owned selected files to the OS Trash / Recycle Bin. It does not offer permanent deletion or remove unrelated files. The confirmation identifies the targets and defaults to Cancel.

If Trash cannot finish, preserve the staged data and recovery journal. Use the offered recovery-folder or retry action; do not manually discard recovery data. After a disk, permission or unavailable-drive error, fix the cause and retry. The app does not silently relocate existing downloads. Damaged or newer saved state is preserved for recovery rather than overwritten with an empty session.

## About and support

Open **Help → About FlowTorrent** or the About button in Settings for the actual package version, author, project and component credits, including the official Slint attribution. The [author website](https://golota.me) is available there. **Support the author** is disabled until the Ko-fi destination is supplied. No payment is required for any feature.
