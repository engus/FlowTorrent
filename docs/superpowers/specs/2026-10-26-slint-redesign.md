[Русский](2026-10-26-slint-redesign-ru.md)
# Native Slint redesign — approved transfer

The user approved the v42 browser prototype and requested its implementation in the existing Rust/Slint client on 26 October 2026. This document records that decision and overrides the older UI exclusions in spec-1.0.md only for the features listed here. AppKit remains isolated; no release, tags, installer uploads or publication are authorized.

## Product behavior
- One cross-platform Slint UI, with the brand kit logo, Nunito font, neutral surfaces, cyan actions and explicit state text as well as color. Keep Made with Slint and all licenses.
- Sidebar: All, Incomplete, Downloaded; Settings at the bottom. Local name search and stable sorting. No network search.
- Start all and Stop all always affect the entire registry, including filtered rows. Per-row ellipsis and the pinned detail footer operate on their named torrent. Single click opens details without checkboxes. Search or modifier selection reveals checkboxes. Selection uses immutable torrent IDs; filtering/removal reconciles selection.
- Progress fills from the left. Incomplete rows show download speed; complete rows show upload speed and uploaded percentage. Progress can reveal an ETA; unknown ETA is explicit. Footer shows green download and red upload arrows.
- A bounded, scrollable detail file list cannot displace the pinned actions. Show actual checked file progress, selected/excluded state and path. Fetch full file detail only for the inspected torrent, not every row. No fake counters.
- Add uses the existing file and magnet preview/confirmation flow, including padding exclusion, duplicate focus, safe destinations and no overwrite.
- Settings: System/Light/Dark theme, Russian/English, launch at login, new-download folder, completion notifications and global rate limits. Rates default to Unlimited, with presets and Custom in Mbit/s. New enabled limits start at 0.5 Mbit/s (62,500 B/s); older valid limits retain their exact value. Existing settings load with safe defaults for the new fields. Save/Cancel are explicit and never relocate payload.
- Keep existing retry behavior. Folder relocation is not introduced by this UI transfer: existing location errors explain the missing folder and Retry rechecks it.
- Remove from list preserves payload. The user explicitly chose OS Trash for Remove torrent and files. Confirmation names immutable targets and the consequence; default action is Cancel. There is no permanent-delete command.

## Architecture and safety
The AppController remains the single registry owner. Use bounded commands and watch snapshots; Slint remains on the main thread. Bulk actions run in the controller, persist intent first, and report failures. Errors never cause blind upstream unpause. Hidden snapshots remain coalesced, and file details are requested only while useful.

Transfer totals come from librqbit. Cumulative upload reporting is session lifetime until a durable cumulative counter is implemented; label this accurately. Keep core minimum-rate validation for backwards compatibility; enforce the new minimum for newly edited UI values.

Trash is a separate application operation; SecureStorage deletion continues to fail. Stop intent is saved before engine release. Only manifest-owned selected payload files are candidates. Securely traverse parents, pin regular files, reject links/reparse/hard links, and compare identity when staging into a private same-volume directory. Verify staged identity before calling OS Trash; never send an unexpected object to Trash. Keep a recovery journal and staged data on interruption or failure, with a visible actionable error. Do not automatically destroy recovery data. Save registry removal only after successful completion. Test with a fake Trash backend and controlled fixtures; actual system integration uses disposable owned fixtures only.

Launch-at-login changes only FlowTorrent's dedicated OS entry; preserve conflicting entries and report failure. Never enable it merely by opening settings or during tests. OS changes happen only after explicit Save and are reverted if saving settings fails.

## Validation contract
Meaningful tests for state/filter/selection, exact rate units and compatibility, bulk intent/failure, actual per-file progress, Trash refusal/failure and settings cancellation. Build and test with locked Rust 1.92 dependencies and -j 2. Validate a real isolated Mac app and lifecycle. Windows x64 and both Mac architectures remain CI targets; distinguish source/CI coverage from physical platform checks. Do not claim native certification from the browser prototype.
