# FlowTorrent 1.0

[Русская версия](spec-1.0-ru.md)

Specification of the first version for Windows and macOS. Date: 21 October 2026. License of FlowTorrent's own code and documentation: MIT.

FlowTorrent is a free torrent client built with Rust, librqbit and Slint. The user opens a `.torrent`, confirms the download and leaves the application running in the background. The main scenario works immediately after installation, with no network setup. There are no ads, accounts or analytics collection.

## 1. Purpose and scope of the version

The first version targets ordinary home use: a few downloads, seeding afterwards and occasional visits to the settings. The priorities are file safety, understandable behavior, a responsive interface and moderate resource usage.

Both operating systems share one core and one Slint interface. Only the necessary system integrations and packaging differ. Linux, separate SwiftUI/WinUI interfaces and WebView are out of scope for 1.0.

Target release matrix: Windows 10 22H2 and Windows 11 on x64; macOS 14 and later on Apple Silicon and Intel. This is the support boundary chosen for the first version, not a statement of Slint's minimum requirements. Testing both Mac architectures is part of release preparation.

## 2. Main user scenario

1. The user opens a `.torrent` with the "Open torrent" button or with Ctrl+O / Cmd+O. Double-clicking in Explorer or Finder opens the file in FlowTorrent if the user has made it the default application.
2. The application reads the metadata and shows the name, total size, download folder, an expandable file list and a "Download" button.
3. By default all user files are selected, and the download folder is the system "Downloads" folder.
4. If needed, the user changes the folder or the file selection and clicks "Download".
5. The download appears in the main window. The user can close the window and keep working on the computer.
6. When the selected files finish, the application shows a system notification and keeps seeding.
7. The user opens the folder with the result or clicks "Pause" to stop seeding.

Downloading and seeding of the torrent do not start until "Download" is clicked. Cancelling the dialog does not add the torrent to the list and does not create download files.

## 3. Main window

The permanent elements are "Open torrent", the torrent list and the entry to settings. An empty window contains a short explanation and a button to open a file. A separate category sidebar and search are not needed in the first version.

Each row shows the name, progress of the selected files, downloaded and selected size, state, and current download and upload speed. Completed torrents stay in the list until the user removes them. Order of addition is preserved across restarts; new torrents appear at the top.

The following actions are available for the selected torrent:

- **Pause / Resume.** Pause stops downloading and seeding of this torrent. After completion it stops seeding.
- **Open folder.** Opens the location of the result in Explorer or Finder; downloaded files are never launched automatically.
- **Remove from list.** After confirmation, stops the torrent and removes it from the application registry. The dialog states explicitly that downloaded files will stay. The torrent does not come back after a restart.
- **Retry.** Available for a recoverable error, for example after reconnecting a drive or fixing access permissions.

States: "Checking", "Connecting", "Downloading", "Waiting for sources", "Paused", "Seeding", "Completed" and "Error". "Completed" is used when the selected files are downloaded but seeding is paused. A lack of sources is not an error.

During Checking, the state label includes the real scan percentage when the engine provides a nonzero scan total, and the progress bar reflects this check. Its denominator is the full volume being scanned, which may differ from the selected download size. Scanned bytes do not count as verified downloaded data or trigger completion. If the scan total is unknown, no percentage is invented. After checking, the bar returns to the verified download progress.

The user does not configure columns, sorting, file priorities or categories. Remaining-time estimates, graphs and peer-list tabs are out of scope for 1.0.

## 4. Adding torrents and file placement

The file list is collapsed initially. When expanded, the user sees relative paths, sizes and selection checkboxes. A scrollable list is enough; a separate folder tree is not required. The selection is stored per torrent. With an empty selection the "Download" button is disabled. Changing the selection after the download has started is not supported in 1.0.

For a multi-file torrent, the result goes into a subfolder named after the torrent inside the chosen folder. For a single-file torrent, it goes into a file inside the chosen folder. The final location is shown in the dialog before confirmation. Changing the folder in one dialog does not change the global default folder.

Re-adding is detected by infohash: the application shows the existing entry, does not start a second download and does not change its folder. If the target files already exist during a new add, the application asks the user to choose another location. Automatic overwriting and importing someone else's partial downloads are out of scope for 1.0. Restoring the application's own saved session is handled separately.

Several files received from the OS are processed one after another, each with its own confirmation. While the application is running, new `.torrent` openings are passed to it; a second instance with an independent session is not started.

## 5. Minimal settings

| Setting | Default | Behavior |
| --- | --- | --- |
| Download folder | The current user's system "Downloads" folder | The user can choose another one. The change applies to new torrents; existing files are not moved. |
| Completion notifications | On | One notification when the selected files finish. If the OS denies notification permission, downloading is not affected. |
| Maximum download speed | Unlimited | A global limit for all torrents; located in the collapsed "Speed" section. |
| Maximum upload speed | Unlimited | A global limit while downloading and seeding; located in the same section. |

Limits are entered in MB/s and may have a fractional part. In the interface, 1 MB/s means 1,000,000 bytes/s. Each field has an "Unlimited" toggle next to it; the minimum enabled limit is 0.02 MB/s (20,000 bytes/s). This is an agreed 1.0 restriction: librqbit 9.0.1 cannot accept a protocol block when the limit is too small. An invalid value is explained and not applied. Limit changes take effect without a restart and do not reset pauses. Both limits are off by default.

Settings are saved when changed and survive restarts. If the saved folder is unavailable, the application explains why and requires choosing an available one before adding; existing downloads are not silently redirected to another folder.

The theme follows the system light or dark theme. The language is Russian for a Russian OS locale and English for all others. There are no separate theme or language switches in 1.0. Paths and file names are not translated.

## 6. Background operation and exit

On Windows, closing the window leaves downloads running in the tray. On macOS, the window closes while the application keeps running and stays available from the Dock and the menu bar icon. The background icon's menu contains "Open FlowTorrent" and "Quit". The first time the window is closed, the application explains once that downloads continue in the background.

"Quit", the system quit command and Cmd+Q on a Mac exit the application after saving state. On exit, connections are closed cleanly and the window does not hang waiting for the network. Reopening the application brings back the main window.

After downloading, seeding continues until pause, removal of the entry or exit. There is no automatic stop by time or ratio. During system sleep transfers are suspended; after wake the application restores connections. Preventing sleep and launch at login are not required in 1.0.

## 7. Recovery and errors

The application stores a copy of the `.torrent` metadata, the folder, the file selection, the desired active/paused state and the engine's recovery data. The original `.torrent` file is not needed to continue after it has been added.

After a normal exit, active torrents resume on the next launch; torrents paused by the user stay paused. After a crash the application checks the required data on disk. Rechecking is acceptable; already verified correct pieces do not need to be downloaded again. Progress may be refined after checking.

Network loss and temporary lack of sources are handled by the engine's reconnection. A full disk, missing permissions, a disconnected external drive and missing files are shown as errors of the affected entry. Other torrents keep working. Once the cause is fixed, the user can retry the operation.

Saving settings and state must be robust against interrupted writes. Damaged state must not be silently overwritten with an empty one: the original file is preserved for recovery and the user receives an understandable message.

The engine verifies piece integrity. Paths from metadata must stay inside the chosen location: absolute paths, escaping via `..` and traversal through existing links must not allow writing outside it. An invalid or unsupported `.torrent` produces an explained error, not a crash.

## 8. Architecture

Components:

- **Slint UI:** main window, adding, settings and error messages.
- **Rust application core:** user commands, a single torrent registry, settings and recovery coordination.
- **librqbit adapter:** session creation, adding files, file selection, pause, resume, limits, statistics and clean shutdown.
- **Platform integration:** file and folder dialogs, background icon, notifications, opening locations and delivering files to the running instance.

The engine runs in Tokio, separate from the interface loop. The UI receives state snapshots and sends commands; network operations, hash checking and long disk operations do not run on the UI thread. Statistics in an open window refresh about twice a second. While the window is hidden, redrawing and unnecessary UI updates stop, while downloads and notifications keep working.

The entry identifier is the infohash. The activity state shown in the UI is kept consistent with the engine; hiding the window does not change it. Changes are persisted before a command's success is shown wherever losing the record would lead to incorrect recovery.

User settings are stored in a small local configuration file; session data in the application data directory. Standard OS locations are used. A separate database, service, HTTP server or web interface is not needed. Dependency versions are pinned in Cargo.lock.

FlowTorrent's own code and documentation are distributed under MIT. Slint is used under its free Royalty-free license, with the badge it requires on the project's public and download pages. Dependency licenses are preserved, and information about them is included in the distribution.

The baseline engine version is stable librqbit 9.0.1. Necessary dependency updates are done separately and re-verified. A custom protocol fork and automatic bandwidth tuning are out of the project's scope.

## 9. Engine compatibility

Mandatory 1.0 support covers BitTorrent v1 `.torrent` files, including single-file, multi-file and private torrents. For private torrents, it must be verified that the engine respects the peer-discovery restrictions. Compatibility with specific private trackers and their client allowlists is not promised.

Pure BitTorrent v2 torrents and a separate implementation of their support are not in the mandatory scope. A hybrid torrent is acceptable if it is handled correctly through v1. An unsupported format is explained to the user. Sequential piece selection is used in the form librqbit provides; no custom piece-selection algorithm is developed.

The first technical check must be: launching on both operating systems, real data exchange, file selection, recovery, speed limits, a private torrent and Slint working together with Tokio. A blocking engine shortcoming found this way is recorded before the interface is expanded; it is not covered up by promising a missing feature.

## 10. Appearance and resources

The interface is compact, with readable fonts, restrained accents and vector icons. The light and dark themes use the same layout. File names must stay readable; long values are visually truncated with a way to see the full text. The main actions are keyboard accessible and have visible focus.

Windows is checked at 100%, 125%, 150% and 200% scaling, and the Mac on Retina and ordinary displays. Constant decorative animations, heavy effects and a custom complex window frame are not needed. File and folder pickers use the system components.

Memory usage is measured on release builds: empty window, several torrents, an active download and a hidden window. The report states the OS, architecture, Slint renderer, workload and memory metric. The 50–150 MB discussed earlier is a preliminary guideline, not a guaranteed limit. The absence of steady memory growth during long operation and repeated window reopening is checked separately.

## 11. 1.0 readiness criteria

1. A clean installation and the main scenario pass on Windows and on both target Mac architectures.
2. Adding via the button and via the OS works, including when the application is already running or hidden.
3. Single-file and multi-file downloads finish with correct hashes; the selected set of files matches the dialog.
4. Re-adding and conflicts with existing files create no duplicates and cause no unexpected overwriting.
5. Pause, resume, completion and seeding are shown consistently. Seeding is stopped with the "Pause" button.
6. After a normal exit and after a forced termination, entries, folders, file selections and manual pauses are restored. Already verified data is reused.
7. Network loss, sleep and wake, low disk space, missing permissions, a disconnected drive and invalid metadata are tested.
8. Changes to both global limits apply to several active torrents. Readings are checked with network overhead and stabilization time taken into account.
9. Removing an entry keeps the files; Quit ends the process, and closing the window leaves background downloads running.
10. An overnight download and seeding test passes, including in the background. The light test workload is five simultaneously active torrents and twenty entries in the list; this is a test scenario, not a product limit.
11. Russian and long names, both themes, scaling and keyboard control of the main actions are tested.
12. A Windows installer and a Mac DMG, a short guide and release notes are prepared. Updating over a previous test version preserves the session and settings. For a public signed release, trusted Windows signing and Developer ID with Apple notarization are set up; test builds are labeled separately.

Automated checks cover state transitions, durable saving, duplicates, path conflicts, settings and recovery. Engine integration checks use a controlled swarm with known data; a public swarm serves as an additional check. System integrations and rendering are checked on real Windows and macOS.

## 12. Beyond 1.0

Magnet links, drag and drop, torrent search, RSS, built-in playback, streaming, remote control, launch at login, auto-update, schedules, categories, complex queues, changing the selection after start, moving downloaded files from the application, deleting files from the application, tracker editing, proxy/VPN settings, detailed network settings, automatic seeding stop and a custom torrent protocol.

New features are added after the main scenario has been verified. 1.0 is complete when this scenario and the readiness criteria are met.

## Technical sources

- [librqbit — library API](https://docs.rs/librqbit/9.0.1/librqbit/).
- [AddTorrentOptions — folder, file selection, pause and existing files](https://docs.rs/librqbit/9.0.1/librqbit/struct.AddTorrentOptions.html).
- [SessionOptions — session persistence, fastresume and global limits](https://docs.rs/librqbit/latest/librqbit/struct.SessionOptions.html).
- [rqbit — supported protocols and features](https://github.com/ikatson/rqbit#features-not-exhaustive).
- [Slint — interface and Rust integration](https://github.com/slint-ui/slint).
