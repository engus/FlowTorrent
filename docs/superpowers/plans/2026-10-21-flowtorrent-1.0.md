[Русская версия](2026-10-21-flowtorrent-1.0-ru.md)

# FlowTorrent 1.0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a small, reliable FlowTorrent 1.0 for Windows and macOS: open a `.torrent`, download the selected files and keep seeding in the background.

**Architecture:** One Rust package with an application library and a desktop binary. Slint runs on the main thread and librqbit in Tokio; commands and state snapshots pass between them. Shared logic is isolated from system dialogs, the tray, Finder and the installer.

**Tech Stack:** Rust edition 2024, Rust 1.92.0 as the initial pinned toolchain; librqbit 9.0.1; Slint/slint-build 1.18.1 with the built-in SystemTrayIcon; Tokio, serde/serde_json, thiserror, tracing, async-trait, directories. System adapters: rfd 0.17.2, the native Windows Toast/UserNotifications APIs, single-instance 0.3.3, interprocess 2.4.6; platform-specific Windows/AppKit APIs where needed. All resolved versions are pinned in Cargo.lock after the first successful build. The toolchain version is raised in a separate change, and only for a confirmed dependency requirement.

**Spec:** [FlowTorrent 1.0 specification](../../spec-1.0.md). Read it before executing the plan; where they disagree, the specification defines the behavior. The plan is not evidence that any check was performed.

## Global Constraints

- Target release matrix: Windows 10 22H2 and Windows 11 on x64; macOS 14 and later on Apple Silicon and Intel.
- Rust + librqbit + Slint, one shared interface. Own code and documentation are MIT; Slint is Royalty-free with the required attribution. Do not replace the author's LICENSE.
- Only opening local `.torrent` files. BitTorrent v1 is mandatory; hybrid torrents use v1; pure v2 is not in the mandatory support.
- Main scenario: open → check folder/selection → "Download" → close the window → get a notification → keep seeding.
- All user files are selected by default; the file list is collapsed. The selection does not change after the download starts.
- The download folder is the current user's system "Downloads" folder. Notifications are on. Both global limits are off and live in the collapsed "Speed" section.
- 1 MB/s means 1,000,000 bytes/s; the minimum active limit is 0.02 MB/s (20,000 bytes/s), agreed after checking librqbit in task 1. Changing the folder affects only new torrents.
- The theme follows the system. Russian for a Russian locale, English for all others; no separate theme/language settings.
- Closing the window keeps background operation. "Quit" and Cmd+Q exit the application. After downloading, seeding continues until pause, removal of the entry or exit.
- The list is ordered by addition, newest entries at the top; the identifier is the infohash. Removing an entry keeps the downloaded files.
- Pauses survive restarts. Re-adding does not create a second download. Other people's existing files are not overwritten.
- A public release requires checks on both operating systems, installers and signatures. Unsigned builds remain test builds.
- Auto-update, launch at login, magnet, RSS, search, player, complex queues and network settings in the UI are absent.

## Review Focus

1. Two files with different names and the same infohash, opened almost simultaneously: one entry and one set of files; tested in task 3.
2. A crash during pause or removal: the confirmed action survives a restart; a removed torrent does not come back; tested in task 5.
3. A disconnected external drive and links in the path: writes do not go to another drive and do not leave the target folder; tested in task 3 and the recovery check in task 5.
4. Opening a `.torrent` via Finder with the window hidden and a repeated launch on Windows: the same process receives the file; checked on real operating systems in task 7.
5. A relaunch after completion: the notification does not arrive again and a manual pause remains; tested in tasks 5 and 8.

## Order and milestones

| Stage | Tasks | Verifiable result |
| --- | --- | --- |
| Technical foundation | 1 | A window and a real download on both operating systems, early check of system events |
| Working main scenario | 2–5 | Safe adding, control, seeding and recovery |
| Interface and background operation | 6–8 | The three UI parts, opening from the OS, tray/menu, notifications |
| Release candidate | 9–10 | Reliability checks, resources, installers, signatures |

The initial estimate is 6–8 full working weeks including a buffer. This is not a date commitment: re-estimate from real results after task 1. Publishing the installers remains a separate final action after the candidate has been verified.

## File structure

One package `flowtorrent`, binary `flowtorrent`, library `flowtorrent`. Do not create a multi-package workspace until there is a confirmed need.

| Files | Responsibility |
| --- | --- |
| Cargo.toml, Cargo.lock, rust-toolchain.toml, build.rs | Rust build and Slint compilation |
| src/main.rs, src/lib.rs | Desktop entry point and modules available to integration tests |
| src/domain.rs, src/error.rs | Types, states, errors with localizable codes |
| src/settings.rs, src/rate.rs, src/paths.rs, src/persistence.rs | The four settings, speed units, application folders and atomic JSON writes |
| src/engine/mod.rs, src/engine/librqbit.rs | The engine boundary and its implementation |
| src/app/mod.rs, src/app/commands.rs, src/app/preview.rs | Registry owner, command handling, file-confirmation queue |
| src/app/storage.rs, src/app/restore.rs | Durable state and recovery |
| src/app/targets.rs | Write-location checks and file ownership |
| src/ui/mod.rs, src/ui/models.rs, src/ui/i18n.rs | Binding Slint to the core, display models and strings |
| ui/main.slint, ui/add-torrent.slint, ui/settings.slint, ui/theme.slint, ui/tray.slint | Main window, adding, settings, the two system themes and the built-in background icon |
| src/platform/mod.rs, src/platform/windows.rs, src/platform/macos.rs | The system integration contract and its OS implementations |
| src/platform/instance.rs, src/platform/notifications.rs | Single instance, file delivery and notifications |
| tests/support/mod.rs, tests/fixtures/* | A controlled local swarm and small synthetic data |
| tests/engine_smoke.rs, tests/engine_contract.rs | Initial check and the engine contract |
| tests/settings.rs, tests/add_torrent.rs, tests/targets.rs | Settings, add preparation, file protection |
| tests/session.rs, tests/restore.rs, tests/crash_recovery.rs | Commands, recovery, crashes |
| tests/ui_models.rs, tests/notifications.rs, tests/instance.rs | UI models and system-event boundaries |
| .github/workflows/ci.yml, .github/workflows/package.yml | Checks, builds and packaging without automatic publication |
| packaging/windows/*, packaging/macos/* | Installer and Mac application |
| docs/engineering-validation.md, docs/release-checklist.md, docs/performance.md | Actual verification results, release and measurements |

## Shared interfaces

The contracts below are FlowTorrent's own API, not a promise of identically named librqbit methods. librqbit calls stay inside the adapter. Do not add unneeded public APIs or plugin abstractions.

In `src/domain.rs`: `TorrentId([u8; 20])`, `FileId(usize)`, `DesiredState::{Active, Paused}`, `TransferState::{Checking, Connecting, Downloading, WaitingForSources, Paused, Seeding, Completed, Error}`. `TorrentId` serializes as 40 hexadecimal characters.

`TransferLimits { download_bps: Option<NonZeroU32>, upload_bps: Option<NonZeroU32> }`; `Settings { download_dir: PathBuf, notify_on_complete: bool, limits: TransferLimits }`. `TorrentFile { id: FileId, relative_path: PathBuf, length: u64, is_padding: bool }`; `TorrentPreview { id: TorrentId, name: String, files: Vec<TorrentFile>, metadata: Vec<u8>, multi_file: bool }`.

`AddRequest { id: TorrentId, base_dir: PathBuf, selected_files: Vec<FileId> }` refers to the preview prepared in the core; the UI cannot substitute the metadata. `TorrentRecord` stores the id, name, order-of-addition position, path to the saved metadata, base_dir, selected files, desired state and `completion_notified: bool`.

`TorrentSnapshot` contains the id, name, selected size, verified downloaded size, speeds in bytes/s, TransferState and an optional `UserError`. `AppSnapshot` contains the ordered rows, the current Settings and window visibility. `UserError { code: ErrorCode, detail: Option<String> }`; the technical detail does not replace the user-facing string.

In `src/error.rs`: `AppError` and `type AppResult<T> = Result<T, AppError>`. Errors distinguish invalid metadata, unsupported format, file conflict, unavailable folder, full disk, permissions, engine error and corrupted saved data.

`TorrentEngine: Send + Sync` via async-trait: `inspect(&self, path: &Path) -> AppResult<TorrentPreview>`, `add(&mut self, preview: &TorrentPreview, request: &AddRequest, mode: AddMode) -> AppResult<TorrentId>`, `pause(&mut self, id: TorrentId) -> AppResult<()>`, `resume(&mut self, id: TorrentId) -> AppResult<()>`, `forget(&mut self, id: TorrentId) -> AppResult<()>`, `snapshot(&self) -> AppResult<Vec<TorrentSnapshot>>`, `set_limits(&mut self, limits: TransferLimits) -> AppResult<()>`, `shutdown(&mut self) -> AppResult<()>`. All of these methods are async. `AddMode::{New, RestorePaused}`; only restoring a trusted record of our own may allow working with already existing files.

`AppCommand::{OpenTorrent(PathBuf), ConfirmAdd(AddRequest), CancelPreview, Pause(TorrentId), Resume(TorrentId), Forget(TorrentId), Retry(TorrentId), UpdateSettings(Settings), SetVisible(bool), Shutdown}`. `AppEvent::{PreviewReady(TorrentPreview), FocusTorrent(TorrentId), Error { id: Option<TorrentId>, error: UserError }, Completion(TorrentId), ShutdownReady}`. The snapshot is delivered only through watch; the current preview and the global error are also available in AppSnapshot. Commands go through a bounded Tokio mpsc and snapshots through watch; Slint handles are not passed between threads.

## Task 1 — Confirm the build and the engine/interface pairing

**Files:** create Cargo.toml, Cargo.lock, rust-toolchain.toml, build.rs, src/main.rs, src/lib.rs, src/engine/mod.rs, src/engine/librqbit.rs, ui/main.slint, tests/support/mod.rs, tests/engine_smoke.rs, .github/workflows/ci.yml, docs/engineering-validation.md.

**Interfaces:** consumes the specification; creates a buildable package and the initial TorrentEngine implementation. Only the domain types from the contract above that are needed are allowed in this task; task 2 adds the rest.

- [x] Install the Rust toolchain and the required Windows C++ build tools if missing; record the versions in the report. Setting up the environment belongs to this stage, not to writing the plan.
- [x] Create a minimal package. For Slint, disable default features and enable `std`, `backend-winit`, `renderer-software`, `compat-1-2`, `system-tray`, `accessibility`. Do not pull in Qt or WebView. Tokio is a multi-thread runtime outside the UI. Check the CPU renderer first; add another renderer only based on the result of that check.
- [x] Write `controlled_swarm_downloads_known_bytes`: a local seeder serves synthetic data and the downloader saves it into a tempdir; the check is `assert_eq!(std::fs::read(destination)?, fixture_bytes);` and no external internet is required. The test must fail first because the implementation is missing.
- [x] Implement a minimal adapter, then run `cargo test --test engine_smoke -- --nocapture` and `cargo run --release`. Check live progress updates without blocking the window.
- [x] Extend the smoke check with selecting a single file, pause/resume, recovery from existing verifiable files, a speed limit and a private torrent. For the private swarm, intercept discovery calls: the private infohash must not leak to DHT/PEX/LSD.
- [ ] Check the Winit/Slint lifecycle, hiding the last window and delivery of a Finder file open to an already running process. Do not postpone discovering AppKit delegate limitations until task 7. Record the working approach in docs/engineering-validation.md; task 7 implements the full user scenario.
- [x] In CI, run build/test on `windows-2022` / x86_64-pc-windows-msvc, `macos-15` / aarch64-apple-darwin and `macos-15-intel` / x86_64-apple-darwin. For Mac, set MACOSX_DEPLOYMENT_TARGET=14.0. Do not make public network torrents a condition of unit/integration CI.
- [ ] Check application startup on a real Mac. Do not record a successful CI build as a check of Finder, Dock, menu or Retina. If a Mac is not available yet, explicitly keep the unverified items in the report; do not declare them done and do not promise a release.
- [x] Run `cargo fmt --all -- --check`, `cargo check --locked --all-targets` and the tests. Record the result and the initial memory of the release build. On a blocking incompatibility, revise the decision before the UI grows instead of writing a custom engine.
- [x] Commit: `feat: establish cross-platform Slint and librqbit foundation`.

**Done:** a window and a real download on Windows/Mac; the limitations of persistence, private swarms, file writes and platform events are known. The test harness remains a verification tool, not a new product feature.

## Task 2 — Types and the four settings

**Files:** create src/domain.rs, src/error.rs, src/settings.rs, src/rate.rs, src/paths.rs, src/persistence.rs, tests/settings.rs; refine src/engine/mod.rs and Cargo.toml.

**Interfaces:** consumes the domain type contract. Creates `Settings::defaults(download_dir: PathBuf) -> Settings`, `parse_rate_mbps(text: &str, unlimited: bool) -> AppResult<Option<NonZeroU32>>`, `SettingsStore::load(&self) -> AppResult<Settings>`, `SettingsStore::save(&self, settings: &Settings) -> AppResult<()>`, `AppPaths::discover() -> AppResult<AppPaths>` and `atomic_write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()>`. Define TorrentRecord and `Manifest { schema_version: u32, next_position: u64, torrents: Vec<TorrentRecord> }` in domain.rs for use by later stages.

- [x] Write tests for defaults and unit conversion: the folder comes from the provided system path, notify=true, both limits=None; `assert_eq!(parse_rate_mbps("1.5", false)?.unwrap().get(), 1_500_000);`, and the same for `1,5`; unlimited=true → None; 0.02 MB/s is accepted; less than 0.02, zero, negative, NaN and overflow → an error without changing the saved value.
- [x] Run `cargo test --test settings`; make sure the missing logic turns the tests red.
- [x] Implement the types and the conversion without rounding to zero. Use the standard user directories via directories; do not build the "Downloads" path from a folder name by hand. A discovery error or missing write access does not create a hidden alternative folder.
- [x] Implement JSON schema_version=1 and durable saving: a temp file in the same directory, flush/sync and an atomic replace that keeps the previous good copy. On Windows, do not delete the old file before rename. Reuse this mechanism for later session writes.
- [x] Check round-trip, saving with an interrupted write and corrupted JSON: the error is understandable, the original bytes are preserved and an empty configuration is not written over them. Run `cargo test --test settings` → PASS.
- [x] Commit: `feat: add minimal settings and shared domain types`.

## Task 3 — Torrent preparation and protection of existing files

**Files:** create src/app/mod.rs, src/app/commands.rs, src/app/preview.rs, src/app/targets.rs, tests/add_torrent.rs, tests/targets.rs; extend tests/fixtures and the adapter.

**Interfaces:** consumes TorrentEngine::inspect and AppPaths. Creates `PreviewQueue::enqueue(path: PathBuf)`, `PreviewQueue::cancel_current()`, `prepare_add(preview: &TorrentPreview, request: &AddRequest) -> AppResult<PreparedAdd>`. `PreparedAdd` contains the verified final location and is the only entry point for creating a new download. `Arc<PreparedAdd>::confirm()` claims the files via create_new and returns an AddRequest with a closed set of file handles. The controller first confirms file ownership, then saves the metainfo and manifest, and only then hands the request to the engine. RestorePaused is allowed only for manifest records; stage 5 implements that check.

- [x] Write the tests `cancel_does_not_create_payload`, `empty_selection_is_rejected`, `same_infohash_is_not_added_twice_concurrently`, `single_and_multi_file_destinations_match_preview` and `existing_payload_is_not_overwritten`. Empty-selection check: `assert!(prepare_add(&preview, &empty_request).is_err());`. For concurrent adding, use the same metadata under different names.
- [x] Run `cargo test --test add_torrent --test targets` and see a red result before implementing.
- [x] Implement the preview queue and the selection check. Filter padding out of the user-facing list while keeping the engine's original file indices. Reserve the infohash on confirmation so that two parallel confirmations cannot both pass the check as new.
- [x] Check metadata before writing: absolute paths, `..`, Windows drive/UNC paths, names that conflict after normalization, symlink/reparse points in the target parents → refuse. `targets_rejects_escape_and_links` checks that a file outside the target is unchanged. Separately check path substitution between preview and the start of writing; string checks done in advance are not enough on their own.
- [x] Implement confirmation of the final location and safe opening/creation of files without clobbering for New. Use the verified protection of engine storage; if it is insufficient, restrict operations with a StorageFactory adapter. For RestorePaused, allow only our own saved record and a recheck of the content.
- [x] Check Cyrillic, long names, an unavailable external drive and unsupported pure v2: an understandable error, with nothing written elsewhere. Rerun both test targets → PASS.
- [x] Commit: `feat: add safe torrent preview and confirmation`.

## Task 4 — Downloading, seeding and commands

**Files:** extend src/app/mod.rs, src/app/commands.rs, src/engine/librqbit.rs; create src/app/storage.rs with the storage contract, tests/session.rs and tests/engine_contract.rs.

**Interfaces:** consumes PreparedAdd, TorrentEngine and SettingsStore. Creates `SessionRepository: Send + Sync` with `load(&self) -> AppResult<Manifest>` and `save(&self, manifest: &Manifest) -> AppResult<()>`, as well as `AppController::spawn(engine: Box<dyn TorrentEngine>, store: Arc<dyn SessionRepository>, settings_store: Arc<SettingsStore>, paths: AppPaths) -> AppHandle`; `AppHandle { commands: mpsc::Sender<AppCommand>, snapshots: watch::Receiver<AppSnapshot>, events: mpsc::Receiver<AppEvent> }`. The in-memory repository lives only in tests/support for controller checks. Task 5 implements the product store; task 6 does the final wiring of the controller into main.

- [x] Write state tests: confirmation adds one row; progress sampling covers only selected_files; pause stops both transfers; completion → Seeding; pause after completion is checked as `assert_eq!(snapshot.state, TransferState::Completed);`; waiting for sources does not turn into Error.
- [x] Run `cargo test --test session --test engine_contract` and record the expected FAILs before implementing.
- [x] Implement a single registry owner with a bounded mpsc, snapshot coalescing and newest-first order. Do not create a separate queue scheduler. When a command races a snapshot, the UI state changes only after the operation's result; a stale snapshot does not cancel a manual pause.
- [x] Apply the global limits through the librqbit session API. The test `two_torrents_share_global_limits` uses two downloader jobs and a local seeder: the limit is shared between them, not multiplied by the number of torrents. Measure payload data after warm-up, separately from protocol overhead.
- [x] Add controlled errors for one torrent and Retry: the other rows keep working. Check `failed_add_keeps_an_explainable_error` and `remove_keeps_payload_and_stops_transfer`.
- [x] Rerun the tests → PASS, then a local download through the minimal window from task 1.
- [x] Commit: `feat: implement torrent lifecycle and global speed limits`.

## Task 5 — Durable session and recovery

**Files:** extend src/app/storage.rs; create src/app/restore.rs, tests/restore.rs, tests/crash_recovery.rs; extend the controller, settings storage and the adapter.

**Interfaces:** `SessionStore` implements SessionRepository from task 4 for the Manifest in domain.rs. Creates async `restore_session(engine: &mut dyn TorrentEngine, manifest: &Manifest, paths: &AppPaths) -> RestoreReport { checking, errors }`. The controller has already loaded the manifest through SessionRepository; the report keeps independent per-record errors. Hash checks run in the engine, and the controller resumes Active only after leaving Checking, consulting the latest saved intent. Real store file operations run via spawn_blocking, preserving the order of controller commands.

- [x] Write `restart_restores_paths_selection_and_pause`, `original_torrent_file_is_not_required`, `confirmed_remove_is_not_resurrected`, `corrupt_manifest_is_preserved`, `completed_notification_stays_acknowledged`. After a confirmed pause: `assert_eq!(store.load()?.torrents[0].desired_state, DesiredState::Paused);`. Run `cargo test --test restore --test crash_recovery` → expected FAIL.
- [x] Make the application manifest the single source of the list and of desired states. Save the original metadata under the infohash in app data. Disable librqbit's built-in automatic list restore so that two databases do not resurrect removed entries.
- [x] Restore trusted records via RestorePaused, check existing pieces, then resume only desired=Active. For 1.0, a correct recheck of existing data is acceptable. Do not add fastresume at the cost of a second source of truth; speed it up separately after correctness is proven.
- [x] Persist the confirmed intent before the UI gets a success response. A pause is shown as successful after the engine confirms it; on error, the error is shown, not an invented pause. Removal first stops the transfer, then durably excludes the record from the manifest; leftover engine metadata does not restore it.
- [x] Implement the same atomic write as SettingsStore, and an understandable state on corruption. Structure upgrades go by schema_version; an unknown future version is not overwritten.
- [x] In a subprocess, terminate the application at points before/after the durable write of Add/Pause/Forget, then relaunch. Check the saved intents, the absence of unrelated overwriting and the reuse of verified pieces. A missing drive stays an error of its record without redirecting the path.
- [x] Rerun the tests → PASS; check a normal restart and forced termination of the core in subprocesses.
- [x] After wiring the UI in task 6, check a normal restart and forced termination of the desktop build on Windows; Mac remains a separate manual check.
- [x] Commit: `feat: persist sessions and recover interrupted downloads`.

## Task 6 — Finish the three parts of the Slint interface

**Files:** create ui/add-torrent.slint, ui/settings.slint, ui/theme.slint, src/ui/mod.rs, src/ui/models.rs, src/ui/i18n.rs, tests/ui_models.rs; extend ui/main.slint and main.

**Interfaces:** consumes AppHandle/AppSnapshot. Creates `UiController::new(handle: AppHandle) -> AppResult<UiController>`, `UiController::window(&self) -> &slint::Window`, `to_row(snapshot: &TorrentSnapshot, locale: Locale) -> TorrentRowModel`, `Locale::{Ru, En}` and `Locale::from_system_tag(tag: &str) -> Locale`. `TorrentRowModel` is a plain Rust display model before conversion to a Slint model. PlatformShell connects to window() in task 7.

- [x] Write tests for the exact RU/EN state strings, Completed after seeding is paused, selected progress, newest-first, and the zero-selection restriction; `assert_eq!(Locale::from_system_tag("ru-RU"), Locale::Ru);`, the same for `ru`, and En is expected for other locales. Check that paths and file names are not translated. `cargo test --test ui_models` FAILs first.
- [x] Implement the main window, the add dialog and a single settings page. The file list and "Speed" are collapsed by default; there are exactly four global settings. No category sidebar and no new features from the excluded scope.
- [x] Bind callbacks to commands through weak handles and invoke_from_event_loop. Keep the selection and scroll position when rows update. Every 500 ms update only the data that changed; the UI timer does not run while the window is hidden.
- [x] Implement errors next to the action, a "Remove from list" confirmation with text saying the files stay, the full text of a long name on hover/accessible expansion, focus and Ctrl+O/Cmd+O. Do not record the folder chosen for one torrent as a new global setting.
- [ ] Check both system themes, RU/EN, the keyboard-only add path and accessibility of the main buttons. On Windows check 100/125/150/200%, on Mac an ordinary display and Retina. Record the results; do not substitute a screenshot from another project.
  - [Mac Slint UI acceptance](../../validation/mac-ui-acceptance-2026-10-23.md): Tab/Enter/Space/Escape and the current dark theme were checked on physical Retina. The user reproduced Cmd+O working on English input but failing on Russian input. The full native-picker keyboard path, reverse navigation/clipboard, other themes/locales/displays remain open.
- [x] Run `cargo test --test ui_models` → PASS and `cargo check --locked --all-targets`; then the manual path with a real download and restart.
- [x] Commit: `feat: build minimal Slint download interface`.

## Task 7 — Opening from the OS, single instance and the background window

**Files:** create src/platform/mod.rs, src/platform/windows.rs, src/platform/macos.rs, src/platform/instance.rs, ui/tray.slint, tests/instance.rs; extend main and UiController.

**Interfaces:** `PlatformEvent::{OpenTorrentPaths(Vec<PathBuf>), Reopen, Quit, ThemeChanged(bool)}`. `InstanceGuard::acquire(paths: &AppPaths) -> AppResult<InstanceRole>`; `InstanceRole::{Primary(InstanceGuard), Secondary}`. `forward_open(paths: &[PathBuf]) -> AppResult<()>`. `PlatformShell::attach(window: &slint::Window, events: std::sync::mpsc::Sender<PlatformEvent>) -> AppResult<PlatformShell>`, `PlatformShell::show_main(&self) -> AppResult<()>`, `PlatformShell::hide_main(&self) -> AppResult<()>`, `PlatformShell::finish_exit(&self) -> AppResult<()>`. The bool in ThemeChanged means dark=true. The system-event bridge passes commands to AppHandle; a full bounded queue is handled without losing Quit and without waiting on the network on the UI thread.

- [x] Write tests/instance.rs: the first process holds the guard, the second passes two paths with spaces/Cyrillic, the guard is not released until exit, and the message arrives once and in the original order: `assert_eq!(received_paths, vec![first_path, second_path]);`. Check the race of launching before the listener is ready: a bounded retry, with no second engine session. Run `cargo test --test instance` → FAIL.
- [x] Use single-instance as the process guard and interprocess for local delivery. Pass structured paths without a shell or command-string parsing; the endpoint is accessible to the current user. Check the message size and number of paths; an invalid message does not terminate the primary.
- [x] On Windows, accept argv on a cold start and IPC on a warm one. On Mac, connect the `openFiles`/reopen event adapter verified in task 1, keeping Slint/Winit and its delegate working. Checking argc alone is not enough.
- [x] Create the built-in Slint SystemTrayIcon/menu on the main thread; do not add a separate tray-icon runtime. CloseRequested hides the window and keeps the event loop alive; use run_event_loop_until_quit for explicit control of shutdown. Quit starts saving/shutdown and calls quit_event_loop only after ShutdownReady. Cmd+Q and Dock reopen follow the same contract.
  - Physical check on 23 October: the user confirmed Dock reopen and Quit from its context menu; process exit was checked independently. Keyboard pause, Cmd+Q and relaunch restored Paused. The menu-bar item remained unverified in that check; [report](../../validation/mac-ui-acceptance-2026-10-23.md).
- [x] Explain background operation once, persisting the fact that it was shown. Check opening from the tray and the Mac menu, closing the last window, reappearance and bringing add dialogs to the foreground.
- [ ] Run `cargo test --test instance` → PASS. On both operating systems check cold/warm launch by double-click, the hidden window and sequential confirmation of several files. After Ctrl+C in a terminal and a normal exit, no background process is left hanging.
- [x] Commit: `feat: integrate native file opening and background lifecycle`.

## Task 8 — Notifications and user-facing errors

**Files:** create src/platform/notifications.rs, tests/notifications.rs; extend src/ui/i18n.rs, src/app/mod.rs, restore and the platform adapters.

**Interfaces:** `NotificationSink::completed(id: TorrentId, name: &str) -> AppResult<()>`; `completion_event(record: &mut TorrentRecord, snapshot: &TorrentSnapshot, enabled: bool) -> Option<CompletionNotice>`. The UI receives an ErrorCode with localized text, and Retry only for a recoverable error.

- [x] Write `one_completion_notice_across_restart`, `disabled_or_denied_notifications_do_not_break_downloads`, `selected_files_completion_is_enough`, `waiting_for_sources_is_not_error`. For a record with completion_notified=true, check `assert!(completion_event(&mut record, &completed_snapshot, true).is_none());`. With notifications off, the completion event is still recorded, and when they are turned on later, old completions are not sent. Run `cargo test --test notifications` → FAIL.
- [x] Persist completion_notified before sending the system notification. Choose at-most-once for a crash between saving and sending: a missed notification is better than repeated messages on every launch. Downloading and seeding do not depend on notification success.
- [x] Connect the native system APIs. A permission error or a missing notification service does not put the torrent into Error. A notification does not automatically launch the downloaded file.
- [x] Add understandable texts for a full disk, a missing volume, permissions, a corrupted `.torrent`, an unsupported format and a corrupted manifest; send technical details to a bounded local log.
- [ ] Run the tests → PASS. Check one completion in the background and a pause surviving a restart on Windows/Mac; check a system denial of notification permission.
  - The installed Slint build on ARM Mac/macOS 27 passed [delivery/system denial and no duplicates](../../validation/mac-notifications-2026-10-23.md), including completion with a closed window; physical Windows checks remain open. Pause restoration was separately checked in [Mac acceptance](../../validation/mac-validation-2026-10-23.md).
- [x] Commit: `feat: add completion notifications and actionable errors`.

## Task 9 — Reliability, resources and the release candidate

On 23 October the installed Slint build on this ARM Mac passed Finder cold/warm opening, background download, pause/resume, real sleep/wake (8 seconds paused and 26 seconds active), completed-file SHA-256, exit/restore and record removal without deleting the payload. [Actual results](../../validation/mac-validation-2026-10-23.md). Source and client were on the same Mac; overnight operation, remote peers/Wi-Fi after wake and Windows remain unverified. This does not close stage 9.

The automated part was verified on 22 October 2026: sources `0c990ba`, Check 39683564507 and Release validation 39683655107 passed on Windows x64 / Mac ARM / Mac Intel. Reports and raw CSV are saved. The stage as a whole is not finished: the overnight run, real sleep/wake and physical checks remain open. See [development-handoff.md](../../development-handoff.md).

**Files:** extend tests/engine_contract.rs, tests/crash_recovery.rs, tests/targets.rs; create docs/performance.md, docs/release-checklist.md and a local measurement harness in examples/measure_resources.rs.

**Interfaces:** consumes the finished main scenario and the test seeder. Creates a reproducible resource-measurement protocol and a report with build/OS/architecture/renderer/metric, not a universal RAM promise.

- [x] Add the missing automated checks: the network dropped/restored through a controlled TCP relay, an unavailable volume and retry, ENOSPC/permission failures on real writes through a test storage wrapper, recheck of a corrupted piece. The system disk was not filled; a real device disconnect is checked separately.
- [x] Run `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets` and `cargo build --locked --release` on the whole matrix → PASS. Check: 86 passing entries each, 0 ignored; the release application/probe were built separately on all three targets.
- [x] Record time-bounded measurements of an empty window, five active torrents, twenty entries, a hidden window and hide/show: Windows private bytes/working set, Mac resident/physical footprint. The raw CSV, environment and medians/peaks are saved; the 300-second hide/show is an extra check, not proof that there are no long-term leaks at all. [Report](../../validation/performance.md).
- [ ] Run an overnight download/seeding and recovery after sleep/forced exit. Check all five Review Focus scenarios. Record problems as separate small commits with a reproducing test.
- [x] Fill in the release checklist with actual results and mark unverified environments. Use the 50–150 MB measurements only as an initial guideline; do not substitute them for results.
- [x] Commit of the automated part: `test: validate release reliability and resource usage`; the individual fixes and reproducing tests are kept in the preceding commits. This item does not close the remaining manual checks of stage 9.

## Task 10 — Installers, licenses and release preparation

Automated packaging was implemented in `372e9b0` with later fixes. Core checks passed Windows x64 / Mac ARM / Mac Intel; both Macs build app/DMG, and Windows passed native install/reinstall/uninstall with data preservation on a disposable CI runner. See the [CI report](../../validation/windows-validation-2026-10-23.md). Version remains 0.1.0; physical checks, cross-version upgrades, signing and public release remain open. The AppKit prototype stays isolated.

**Files:** create packaging/windows/flowtorrent.iss, packaging/macos/Info.plist, packaging/macos/entitlements.plist, scripts/package-windows.ps1, scripts/package-macos.sh, .github/workflows/package.yml, THIRD_PARTY_NOTICES.md and CHANGELOG.md; update README.md and the release checklist.

**Interfaces:** input is the verified release binary and Cargo.lock. Output is a per-user Windows installer EXE and separate DMGs for aarch64/x86_64 Mac, checksums and instructions. At first the workflow only builds artifacts; there is no GitHub Release publication on push/tag.

- [x] Package Windows with Inno Setup and Mac as `.app` + `hdiutil`. The binary, bundle and installer versions match. Application identifier `com.engus.flowtorrent`; display name FlowTorrent. On Mac the deployment target is 14.0, and both architectures are packaged and checked separately.
- [x] Register the `.torrent` handler: Windows ProgID `FlowTorrent.Torrent`, Mac BitTorrent UTI and the Viewer role. Do not force the default app: the user chooses it with system tools. Add application identity for native notifications.
- [ ] Collect license notices from the exact Cargo.lock. Keep the author's MIT, the Apache/MIT/other dependency texts and Slint Royalty-free, as well as the visible official badge in the README and the download page template. Do not declare the whole dependency chain MIT.
- [ ] Connect Windows signing and Developer ID/notarization through protected environment/CI credentials. Certificates, passwords and tokens must not end up in the repository or logs. If credentials are unavailable, build explicitly labeled test artifacts and leave the public signed release item unfinished.
- [ ] Check installation on a clean system, opening a `.torrent`, background downloads, updating over a previous test build while preserving the session, and uninstalling without deleting downloaded data. Test release builds after signing/packaging.
- [ ] Update the README: actual status, installation, a short scenario, manual updating, known v1/private limitations and a contact for bugs. Prepare the CHANGELOG and SHA-256. Match each of the specification's 12 criteria against a verification record.
- [x] Packaging commit: `372e9b0` (`build: add isolated Slint test packages and locked license notices`) and later fixes; this does not close all of stage 10.

**Done:** both platforms pass the specification, and the artifacts and notes are ready for a separate publication. Version 1.0.0 is not declared ready on the basis of one successful local build.

## Specification coverage check

| Specification section | Implementation and verification |
| --- | --- |
| 1 — platforms and scope | 1, 9, 10 |
| 2 — main scenario | 3, 4, 6, 7, 8 |
| 3 — main window and states | 4, 6 |
| 4 — files, duplicates, folders | 3, 5, 7 |
| 5 — four settings, themes, language | 2, 4, 6, 8 |
| 6 — background, exit, sleep, seeding | 4, 5, 7, 9 |
| 7 — recovery and errors | 3, 5, 8, 9 |
| 8 — architecture and MIT | 1, 2, 4, 5, 6, 10 |
| 9 — engine compatibility | 1, 3, 4, 9 |
| 10 — appearance and resources | 6, 9 |
| 11 — release criteria | 9, 10 |
| 12 — excluded features | Global Constraints, review of every change |

## Executing the plan

By default the tasks run sequentially: neighbouring stages depend on shared types and engine behavior. Before implementation, the user reviews the plan and chooses execution in the current session or by separate executors with review. Reviewing this document on its own does not require launching subagents.

The recommended option is execution in the current session, with a review after each stage and separate commits. Publishing a release is not an automatic consequence of permission to make commits. Specification changes and discovered limitations are recorded in the repository together with the corresponding decision.

## Sources for implementation

- [librqbit 9.0.1 API](https://docs.rs/librqbit/9.0.1/librqbit/).
- [Session 9.0.1 source](https://github.com/ikatson/rqbit/blob/v9.0.1/crates/librqbit/src/session.rs) — the persistence list is restored automatically when a session is created; do not enable it as an independent registry.
- [Slint Rust API](https://docs.slint.dev/latest/docs/rust/slint/) and [Cargo features 1.18.1](https://github.com/slint-ui/slint/blob/v1.18.1/api/rs/slint/Cargo.toml).
- [GitHub runner images](https://github.com/actions/runner-images) — available macOS and Windows architectures.
- [Slint Royalty-free](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md).
