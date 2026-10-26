use super::{
    components::{DetailFileRow, MainWindow, PreviewFileRow, TorrentRow, Translations},
    i18n::{Locale, error_message, format_bytes},
    models::{PreviewSelection, display_path, to_row},
};
use crate::{
    app::{
        AppEvent, AppHandle, AppSnapshot,
        commands::{AppCommand, AppOperation},
    },
    domain::*,
    error::AppResult,
    platform::{
        PlatformEvent, PlatformShell,
        notifications::{
            NativeNotificationSink, NoopNotificationSink, NotificationDispatcher, NotificationSink,
        },
        state::BackgroundNotice,
    },
    rate::{parse_rate_mbit, rate_mbit_text},
    settings::{LanguagePreference, Settings, ThemePreference, UiLayout},
};
use slint::{ComponentHandle, Model};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::mpsc;

#[derive(Default)]
struct Inbox {
    latest: Option<Arc<AppSnapshot>>,
    events: Vec<AppEvent>,
}
struct UiState {
    locale: Locale,
    system_locale: Locale,
    notification_sink: Arc<dyn NotificationSink>,
    shell: Option<Rc<PlatformShell>>,
    view: super::view_state::ViewState,
    detail_files: Rc<slint::VecModel<DetailFileRow>>,
    detail_requested: Option<TorrentId>,
    removal: Option<(Vec<TorrentId>, bool)>,
    collapsed_folders: std::collections::BTreeSet<String>,
    commands: mpsc::Sender<AppCommand>,
    snapshot: Option<Arc<AppSnapshot>>,
    rows: Rc<slint::VecModel<TorrentRow>>,
    files: Rc<slint::VecModel<PreviewFileRow>>,
    preview_id: Option<TorrentId>,
    selection: Option<PreviewSelection>,
    preview_folder: PathBuf,
    settings_folder: PathBuf,
    last_layout: Option<UiLayout>,
    pending: Option<AppOperation>,
    settings_from_form: bool,
    auto_select: Option<TorrentId>,
    local_error: Option<String>,
}
pub struct UiController {
    component: MainWindow,
    _state: Rc<RefCell<UiState>>,
    pump: tokio::task::JoinHandle<()>,
}

impl UiController {
    pub fn new(handle: AppHandle) -> AppResult<Self> {
        let locale = Locale::from_system_tag(&sys_locale::get_locale().unwrap_or_default());
        Self::with_notifications(
            handle,
            locale,
            Arc::new(NativeNotificationSink::new(locale)),
        )
    }
    /// Explicit locale and silent notification sink for controlled visual checks.
    /// The normal app follows the OS and uses native completion notifications.
    pub fn new_with_locale(handle: AppHandle, locale: Locale) -> AppResult<Self> {
        Self::with_notifications(handle, locale, Arc::new(NoopNotificationSink))
    }
    fn with_notifications(
        handle: AppHandle,
        locale: Locale,
        sink: Arc<dyn NotificationSink>,
    ) -> AppResult<Self> {
        let notifications = match NotificationDispatcher::new(sink.clone()) {
            Ok(dispatcher) => Some(dispatcher),
            Err(error) => {
                tracing::warn!(%error, "Completion notification worker unavailable");
                None
            }
        };
        let runtime = tokio::runtime::Handle::try_current().map_err(anyhow::Error::from)?;
        let component = MainWindow::new().map_err(anyhow::Error::from)?;
        component.set_ru(locale == Locale::Ru);
        component.set_macos(cfg!(target_os = "macos"));
        {
            use slint::winit_030::{EventResult, WinitWindowAccessor};
            let weak = component.as_weak();
            component.window().on_winit_window_event(move |_, event| {
                if let Some(app) = weak.upgrade() {
                    native_theme_event(&app, event);
                }
                EventResult::Propagate
            });
        }
        let rows = Rc::new(slint::VecModel::default());
        let files = Rc::new(slint::VecModel::default());
        component.set_rows(rows.clone().into());
        component.set_preview_files(files.clone().into());
        let commands = handle.commands.clone();
        let state = Rc::new(RefCell::new(UiState {
            locale,
            system_locale: locale,
            notification_sink: sink,
            shell: None,
            view: Default::default(),
            detail_files: Rc::new(slint::VecModel::default()),
            detail_requested: None,
            removal: None,
            collapsed_folders: Default::default(),
            commands: commands.clone(),
            snapshot: None,
            rows,
            files,
            preview_id: None,
            selection: None,
            preview_folder: PathBuf::new(),
            settings_folder: PathBuf::new(),
            last_layout: None,
            pending: None,
            settings_from_form: false,
            auto_select: None,
            local_error: None,
        }));
        component.set_detail_files(state.borrow().detail_files.clone().into());
        bind_callbacks(&component, &state);
        component.on_shutdown_ready(|| {
            let _ = slint::quit_event_loop();
        });
        let inbox = Arc::new(Mutex::new(Inbox::default()));
        let scheduled = Arc::new(AtomicBool::new(false));
        {
            let weak = component.as_weak();
            let state = state.clone();
            let inbox = inbox.clone();
            let scheduled = scheduled.clone();
            component.on_synchronize(move || {
                let Some(app) = weak.upgrade() else {
                    return;
                };
                let (snapshot, events) = {
                    let mut inbox = inbox.lock().unwrap();
                    // Clear under the mailbox lock so a final hidden update cannot lose its wake.
                    scheduled.store(false, Ordering::Release);
                    (inbox.latest.clone(), std::mem::take(&mut inbox.events))
                };
                if let Some(snapshot) = snapshot {
                    let mut state = state.borrow_mut();
                    if snapshot.visible || app.window().is_visible() || !events.is_empty() {
                        state.apply(&app, snapshot);
                    } else {
                        state.sync_locale(&app, &snapshot);
                        state.update_tray(&snapshot);
                    }
                }
                for event in events {
                    state.borrow_mut().event(&app, event);
                }
            });
        }
        {
            let weak = component.as_weak();
            let state = state.clone();
            component.on_native_error(move |message| {
                if let Some(app) = weak.upgrade() {
                    state.borrow_mut().local_error = Some(message.to_string());
                    app.set_error(message);
                }
            });
        }
        {
            let weak = component.as_weak();
            let commands = commands.clone();
            let runtime = runtime.clone();
            component.window().on_close_requested(move || {
                if let Some(app) = weak.upgrade() {
                    app.set_stopping(true);
                }
                let commands = commands.clone();
                runtime.spawn(async move {
                    let _ = commands.send(AppCommand::Shutdown).await;
                });
                slint::CloseRequestResponse::KeepWindowShown
            });
        }
        let weak = component.as_weak();
        let pump = runtime.spawn(async move {
            let mut snapshots = handle.snapshots;
            let mut events = handle.events;
            let mut tray_refresh=super::tray_status::TrayRefresh::default();
            if snapshots.has_changed().unwrap_or(false) {
                let snapshot = Arc::new(snapshots.borrow_and_update().clone());
                inbox.lock().unwrap().latest = Some(snapshot);
                schedule(&weak, &scheduled);
            }
            loop {
                let force = tokio::select! {
                    result = snapshots.changed() => {
                        if result.is_err() {
                            quit_ui(&weak);
                            break;
                        }
                        let snapshot = Arc::new(snapshots.borrow_and_update().clone());
                        let changed=tray_refresh.changed(&snapshot.torrents,Locale::from_preference(snapshot.settings.ui.language,locale));
                        let needed=snapshot.visible || changed;
                        inbox.lock().unwrap().latest = Some(snapshot);
                        needed
                    }
                    event = events.recv() => {
                        match event {
                            Some(AppEvent::ShutdownReady) | None => {
                                quit_ui(&weak);
                                break;
                            }
                            Some(AppEvent::Completion(notice)) => {
                                if let Some(dispatcher) = &notifications {
                                    dispatcher.completed(notice);
                                }
                                false
                            }
                            Some(event) => {
                                let snapshot = Arc::new(snapshots.borrow_and_update().clone());
                                let mut inbox = inbox.lock().unwrap();
                                inbox.latest = Some(snapshot);
                                inbox.events.push(event);
                                true
                            }
                        }
                    }
                };
                if force {
                    schedule(&weak, &scheduled);
                }
            }
        });
        Ok(Self {
            component,
            _state: state,
            pump,
        })
    }
    pub fn window(&self) -> &slint::Window {
        self.component.window()
    }
    /// The visual harness can verify both palettes without changing OS settings.
    pub fn preview_theme(&self, dark: bool) {
        self.component.invoke_preview_color_scheme(dark);
    }
    pub fn enable_background(
        &self,
        shell: Rc<PlatformShell>,
        events: std::sync::mpsc::Receiver<PlatformEvent>,
        notice_path: PathBuf,
    ) -> AppResult<()> {
        self._state.borrow_mut().shell = Some(shell.clone());
        let notice = Rc::new(RefCell::new(BackgroundNotice::load(notice_path)?));
        let weak = self.component.as_weak();
        let state = self._state.clone();
        {
            let shell = shell.clone();
            self.component.on_shutdown_ready(move || {
                if let Err(error) = shell.finish_exit() {
                    tracing::warn!(%error, "Native exit cleanup failed");
                }
                let _ = slint::quit_event_loop();
            });
        }
        {
            let weak = weak.clone();
            let state = state.clone();
            let shell = shell.clone();
            self.component.window().on_close_requested(move || {
                let Some(app) = weak.upgrade() else { return slint::CloseRequestResponse::KeepWindowShown; };
                if app.get_stopping() { return slint::CloseRequestResponse::KeepWindowShown; }
                if !notice.borrow().was_shown() {
                    let locale = state.borrow().locale;
                    let description = if cfg!(target_os = "macos") {
                        locale.text(
                            "Загрузки продолжатся в фоне. Открыть FlowTorrent можно через Dock или значок в строке меню. Чтобы завершить работу, выберите «Выйти» или нажмите Cmd+Q.",
                            "Downloads will continue in the background. Reopen FlowTorrent from the Dock or menu bar icon. Choose Quit or press Cmd+Q to exit.",
                        )
                    } else {
                        locale.text(
                            "Загрузки продолжатся в фоне. Чтобы вернуть окно, нажмите значок FlowTorrent в трее. Для завершения работы выберите «Выйти» в его меню.",
                            "Downloads will continue in the background. Click the FlowTorrent tray icon to reopen the window. Choose Quit in its menu to exit.",
                        )
                    };
                    rfd::MessageDialog::new()
                        .set_parent(&app.window().window_handle())
                        .set_title("FlowTorrent")
                        .set_description(description)
                        .show();
                    if let Err(error) = notice.borrow_mut().mark_shown() {
                        app.invoke_native_error(error.to_string().into());
                        return slint::CloseRequestResponse::KeepWindowShown;
                    }
                }
                if let Err(error) = shell.hide_main() { app.invoke_native_error(error.to_string().into()); }
                slint::CloseRequestResponse::KeepWindowShown
            });
        }
        // A single non-UI receiver preserves native-event/batch ordering. Only
        // this thread waits for bounded core-channel capacity; Quit is never lost.
        let ui_commands = state.borrow().commands.clone();
        std::thread::Builder::new()
            .name("flowtorrent-native-events".into())
            .spawn(move || {
                while let Ok(event) = events.recv() {
                    let (event, delivered) = match event {
                        PlatformEvent::QueuedOpen { paths, delivered } => (
                            if paths.is_empty() {
                                PlatformEvent::Reopen
                            } else {
                                PlatformEvent::OpenTorrentPaths(paths)
                            },
                            Some(delivered),
                        ),
                        event => (event, None),
                    };
                    let (command, paths) = match &event {
                        PlatformEvent::Quit => (Some(AppCommand::Shutdown), Vec::new()),
                        PlatformEvent::Hide => (Some(AppCommand::SetVisible(false)), Vec::new()),
                        PlatformEvent::ThemeChanged(_) => (None, Vec::new()),
                        PlatformEvent::OpenTorrentPaths(paths) => {
                            (Some(AppCommand::SetVisible(true)), paths.clone())
                        }
                        _ => (Some(AppCommand::SetVisible(true)), Vec::new()),
                    };
                    if weak
                        .upgrade_in_event_loop(move |app| match event {
                            PlatformEvent::Quit => app.set_stopping(true),
                            PlatformEvent::Hide => {
                                let _ = app.hide();
                            }
                            PlatformEvent::ThemeChanged(dark) => {
                                app.invoke_preview_color_scheme(dark)
                            }
                            event => {
                                if app.get_stopping() {
                                    return;
                                }
                                let _ = app.show();
                                app.invoke_synchronize();
                                crate::platform::focus(app.window());
                                if matches!(event, PlatformEvent::OpenDialog) {
                                    app.invoke_open_requested();
                                }
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                    if let Some(command) = command
                        && ui_commands.blocking_send(command).is_err()
                    {
                        break;
                    }
                    for path in paths {
                        if ui_commands
                            .blocking_send(AppCommand::OpenTorrent(path))
                            .is_err()
                        {
                            return;
                        }
                    }
                    if let Some(delivered) = delivered {
                        let _ = delivered.send(());
                    }
                }
            })?;
        Ok(())
    }
}
impl Drop for UiController {
    fn drop(&mut self) {
        self.pump.abort();
    }
}
fn quit_ui(weak: &slint::Weak<MainWindow>) {
    let _ = weak.upgrade_in_event_loop(|app| app.invoke_shutdown_ready());
}
fn schedule(weak: &slint::Weak<MainWindow>, scheduled: &Arc<AtomicBool>) {
    if !scheduled.swap(true, Ordering::AcqRel) {
        let scheduled = scheduled.clone();
        if weak
            .upgrade_in_event_loop(|app| app.invoke_synchronize())
            .is_err()
        {
            scheduled.store(false, Ordering::Release);
        }
    }
}
fn native_theme_event(app: &MainWindow, event: &slint::winit_030::winit::event::WindowEvent) {
    use slint::winit_030::winit::{event::WindowEvent, window::Theme};
    if let WindowEvent::ThemeChanged(theme) = event {
        app.invoke_preview_color_scheme(*theme == Theme::Dark);
    }
}
fn parse_id(text: &str) -> Option<TorrentId> {
    serde_json::from_value(serde_json::Value::String(text.into())).ok()
}
fn ui_row(row: super::models::TorrentRowModel) -> TorrentRow {
    TorrentRow {
        id: row.id.to_string().into(),
        name: row.name.into(),
        status: row.status.into(),
        volume: row.volume.into(),
        download: row.download.into(),
        upload: row.upload.into(),
        progress: row.progress_permille as f32 / 1000.0,
        action: row.action.into(),
        error: row.error.unwrap_or_default().into(),
        retry: row.can_retry,
        can_act: row.can_act,
        ..Default::default()
    }
}
impl UiState {
    fn sync_locale(&mut self, app: &MainWindow, snapshot: &AppSnapshot) {
        self.locale = Locale::from_preference(snapshot.settings.ui.language, self.system_locale);
        if app.global::<Translations>().get_locale() != self.locale.index() {
            app.global::<Translations>().set_locale(self.locale.index());
            sync_locale_copy(app, self.locale);
        }
        app.set_ru(self.locale == Locale::Ru);
        sync_category_labels(app, self.locale);
        self.notification_sink.set_locale(self.locale);
        if let Some(shell) = &self.shell {
            shell.set_locale(self.locale);
        }
    }
    fn update_tray(&self, snapshot: &AppSnapshot) {
        if let Some(shell) = &self.shell {
            shell.set_status(&super::tray_status::TrayStatus::from_rows(
                &snapshot.torrents,
                self.locale,
            ));
        }
    }
    fn apply(&mut self, app: &MainWindow, snapshot: Arc<AppSnapshot>) {
        self.sync_locale(app, &snapshot);
        self.update_tray(&snapshot);
        app.invoke_apply_theme(theme_index(snapshot.settings.ui.theme));
        let preview_id = snapshot.preview.as_ref().map(|p| p.id);
        if preview_id.is_some() || snapshot.resolving_magnet {
            app.set_about_open(false);
        }
        if preview_id != self.preview_id {
            self.preview_id = preview_id;
            self.selection = snapshot.preview.as_ref().map(PreviewSelection::new);
            self.preview_folder = snapshot.settings.download_dir.clone();
            app.set_files_expanded(false);
            if let Some(preview) = &snapshot.preview {
                app.set_preview_name(preview.name.clone().into());
                self.files.set_vec(
                    self.selection
                        .as_ref()
                        .unwrap()
                        .files()
                        .map(|file| PreviewFileRow {
                            id: i32::try_from(file.id.0).unwrap_or(i32::MAX),
                            name: file.relative_path.to_string_lossy().into_owned().into(),
                            size: format_bytes(file.length, self.locale).into(),
                            checked: true,
                        })
                        .collect::<Vec<_>>(),
                );
            } else {
                self.files.set_vec(vec![]);
            }
        }
        let layout = snapshot.settings.ui.layout.normalized();
        if self.last_layout != Some(layout) {
            apply_layout_fields(app, layout);
            self.last_layout = Some(layout);
        }
        self.snapshot = Some(snapshot.clone());
        if app.get_show_settings() && self.pending != Some(AppOperation::UpdateSettings) {
            sync_settings_fields(app, self, &snapshot.settings);
        }
        let mut warning = snapshot
            .settings_recovery
            .as_ref()
            .map(|recovery| super::i18n::settings_recovery_message(recovery, self.locale))
            .unwrap_or_default();
        if let Some(path) = &snapshot.blocked_state_path {
            warning = format!("{}\n{}\n{}", self.locale.text("Сохранённая сессия недоступна и оставлена без изменений:", "The saved session could not be read and remains unchanged:"), path.display(), self.locale.text("Сохраните копию повреждённого файла и восстановите исправную резервную копию перед перезапуском.", "Keep a copy of the damaged file and restore a known-good backup before restarting."));
        }
        app.set_settings_warning(warning.into());
        self.sync_preview(app);
        app.set_preview_open(preview_id.is_some());
        app.set_resolving_magnet(snapshot.resolving_magnet);
        if preview_id.is_some() {
            app.set_show_magnet(false);
        } else if snapshot.resolving_magnet {
            app.set_show_settings(false);
            app.set_show_magnet(true);
        }
        app.set_ready(true);
        if let Some(id) = self
            .auto_select
            .filter(|id| snapshot.torrents.iter().any(|r| r.id == *id))
        {
            self.view.category = super::view_state::Category::All;
            self.view.search.clear();
            app.set_search_query("".into());
            self.view.selected.clear();
            self.view.selected.insert(id);
            self.view.details_open = true;
            self.auto_select = None;
            app.set_scroll_y(0.0);
        }
        self.refresh_view(app);
        let error = self.local_error.clone().unwrap_or_else(|| {
            snapshot
                .error
                .as_ref()
                .map(|e| error_message(&e.code, self.locale).to_owned())
                .unwrap_or_default()
        });
        app.set_error(error.into());
    }
    fn select(&mut self, app: &MainWindow, id: &str) {
        if let Some(id) = parse_id(id) {
            self.view.details_open = true;
            self.view.selected.clear();
            self.view.selected.insert(id);
        }
        if self.snapshot.is_some() {
            self.refresh_view(app);
        } else {
            let selected = (0..self.rows.row_count())
                .filter_map(|i| self.rows.row_data(i))
                .find(|row| row.id == id)
                .unwrap_or_default();
            app.set_selected_row(selected);
        }
    }
    fn refresh_view(&mut self, app: &MainWindow) {
        use super::view_state::{self, Sort};
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        self.view.search = app.get_search_query().to_string();
        self.view.search_open = app.get_search_open();
        self.view.sort = match app.get_sort_index() {
            1 => Sort::Name,
            2 => Sort::Size,
            3 => Sort::Progress,
            4 => Sort::Speed,
            _ => Sort::Added,
        };
        let ids = self.view.visible(&snapshot.torrents);
        self.view.reconcile(&ids);
        let next: Vec<_> = ids
            .iter()
            .filter_map(|id| snapshot.torrents.iter().find(|r| r.id == *id))
            .map(|r| {
                let mut row = ui_row(to_row(r, self.locale));
                let unknown_total = r.state == TransferState::Error && r.selected_bytes == 0;
                row.metrics_known = !unknown_total;
                row.size = if unknown_total {
                    "—".into()
                } else {
                    format_bytes(r.selected_bytes, self.locale).into()
                };
                row.downloaded = if unknown_total {
                    "—".into()
                } else {
                    format_bytes(
                        r.checked_downloaded_bytes.min(r.selected_bytes),
                        self.locale,
                    )
                    .into()
                };
                row.selected = self.view.selected.contains(&r.id);
                row.complete = view_state::complete(r);
                row.paused = r.desired_state == DesiredState::Paused;
                row.stage = if r.state == TransferState::Error {
                    4
                } else if r.state == TransferState::Checking {
                    5
                } else if r.desired_state == DesiredState::Paused {
                    0
                } else if row.complete {
                    2
                } else if matches!(
                    r.state,
                    TransferState::WaitingForSources | TransferState::Connecting
                ) {
                    3
                } else {
                    1
                };
                row.flow = if row.complete {
                    row.upload.clone()
                } else {
                    row.download.clone()
                };
                row.eta = if r.state == TransferState::Error {
                    self.locale.text("ETA —", "ETA —").into()
                } else {
                    match view_state::eta_seconds(
                        r.selected_bytes,
                        r.checked_downloaded_bytes,
                        if r.desired_state == DesiredState::Active {
                            r.download_bps
                        } else {
                            0
                        },
                    ) {
                        Some(0) => self.locale.text("Скачано", "Downloaded").into(),
                        Some(seconds) => format!(
                            "ETA {}:{:02}:{:02}",
                            seconds / 3600,
                            seconds / 60 % 60,
                            seconds % 60
                        )
                        .into(),
                        None => self.locale.text("ETA —", "ETA —").into(),
                    }
                };
                row.ratio = match view_state::uploaded_percent(
                    r.session_uploaded_bytes,
                    r.checked_downloaded_bytes,
                ) {
                    Some(p) => format!(
                        "{} {p}%",
                        self.locale.text("Отдано за сеанс", "Uploaded this session")
                    ),
                    None => self
                        .locale
                        .text("Отдано за сеанс —", "Uploaded this session —")
                        .into(),
                }
                .into();
                row.destination = display_path(&r.destination).into();
                row.action = if r.state == TransferState::Error {
                    self.locale.text("Повторить", "Retry")
                } else if r.desired_state == DesiredState::Paused {
                    self.locale.text("Начать", "Start")
                } else {
                    self.locale.text("Остановить", "Stop")
                }
                .into();
                row
            })
            .collect();
        let same_order = self.rows.row_count() == next.len()
            && next
                .iter()
                .enumerate()
                .all(|(i, r)| self.rows.row_data(i).is_some_and(|old| old.id == r.id));
        if same_order {
            for (i, row) in next.iter().enumerate() {
                if self.rows.row_data(i).as_ref() != Some(row) {
                    self.rows.set_row_data(i, row.clone());
                }
            }
        } else {
            self.rows.set_vec(next.clone());
        }
        let count = self.view.selected.len();
        let mut selected = next
            .iter()
            .find(|r| r.selected)
            .cloned()
            .unwrap_or_default();
        if count > 1 {
            selected.name = format!("{} {count}", self.locale.text("Выбрано:", "Selected:")).into();
            selected.id = "".into();
            selected.can_act = true;
            let stopped = snapshot
                .torrents
                .iter()
                .filter(|r| self.view.selected.contains(&r.id))
                .all(|r| r.desired_state == DesiredState::Paused);
            selected.stage = if stopped { 0 } else { 1 };
            selected.paused = stopped;
            selected.action = self
                .locale
                .text(
                    if stopped {
                        "Начать"
                    } else {
                        "Остановить"
                    },
                    if stopped { "Start" } else { "Stop" },
                )
                .into();
            selected.flow = "".into();
            selected.volume = "".into();
            selected.status = "".into();
            selected.ratio = "".into();
            selected.destination = "".into();
        }
        app.set_selected_row(selected);
        app.set_selected_count(count as i32);
        app.set_details_open(self.view.details_open && count > 0);
        app.set_checkboxes(self.view.checkboxes());
        app.set_category(match self.view.category {
            view_state::Category::All => 0,
            view_state::Category::Incomplete => 1,
            view_state::Category::Downloaded => 2,
        });
        let downloaded = snapshot
            .torrents
            .iter()
            .filter(|r| view_state::complete(r))
            .count();
        app.set_total_count(snapshot.torrents.len() as i32);
        app.set_downloaded_count(downloaded as i32);
        app.set_incomplete_count((snapshot.torrents.len() - downloaded) as i32);
        let suffix = self.locale.text("/с", "/s");
        app.set_total_download(
            format!(
                "{}{suffix}",
                format_bytes(
                    snapshot
                        .torrents
                        .iter()
                        .fold(0u64, |s, r| s.saturating_add(r.download_bps)),
                    self.locale
                )
            )
            .into(),
        );
        app.set_total_upload(
            format!(
                "{}{suffix}",
                format_bytes(
                    snapshot
                        .torrents
                        .iter()
                        .fold(0u64, |s, r| s.saturating_add(r.upload_bps)),
                    self.locale
                )
            )
            .into(),
        );
        let single = self.view.inspected().filter(|_| {
            snapshot.visible
                && !app.get_show_settings()
                && !app.get_preview_open()
                && !app.get_show_magnet()
        });
        if single != self.detail_requested
            && self
                .commands
                .try_send(AppCommand::InspectDetails(single))
                .is_ok()
        {
            self.detail_requested = single;
            app.set_file_query("".into());
            self.collapsed_folders.clear();
        }
        let query = app.get_file_query().to_lowercase();
        let mut files = Vec::new();
        let details = snapshot.details.as_ref().filter(|d| Some(d.id) == single);
        if let Some(details) = details {
            let selected = details.files.iter().filter(|f| f.selected).count();
            app.set_detail_summary(format!("{selected}/{}", details.files.len()).into());
            let mut groups = std::collections::BTreeMap::<String, Vec<&FileProgress>>::new();
            for file in &details.files {
                if query.is_empty() || file.path.to_string_lossy().to_lowercase().contains(&query) {
                    let folder = file.path.parent().map(display_path).unwrap_or_default();
                    groups.entry(folder).or_default().push(file);
                }
            }
            for (folder, entries) in groups {
                let collapsed = query.is_empty() && self.collapsed_folders.contains(&folder);
                if !folder.is_empty() {
                    files.push(DetailFileRow {
                        name: folder.clone().into(),
                        path: folder.into(),
                        group: true,
                        collapsed,
                        ..Default::default()
                    });
                }
                if collapsed {
                    continue;
                }
                for file in entries {
                    files.push(DetailFileRow {
                        name: file
                            .path
                            .file_name()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_default()
                            .into(),
                        path: display_path(&file.path).into(),
                        size: format_bytes(file.length, self.locale).into(),
                        selected: file.selected,
                        progress: if file.length == 0 {
                            if file.selected { 1.0 } else { 0.0 }
                        } else {
                            file.checked_bytes as f32 / file.length as f32
                        },
                        status: if !file.selected {
                            self.locale.text("Не выбран", "Excluded").into()
                        } else if file.length == 0 || file.checked_bytes >= file.length {
                            self.locale.text("Скачан", "Downloaded").into()
                        } else {
                            format!(
                                "{}%",
                                u128::from(file.checked_bytes) * 100 / u128::from(file.length)
                            )
                            .into()
                        },
                        ..Default::default()
                    });
                }
            }
        } else {
            app.set_detail_summary("".into());
        }
        if self.detail_files.iter().collect::<Vec<_>>() != files {
            self.detail_files.set_vec(files);
        }
    }
    fn sync_preview(&self, app: &MainWindow) {
        if let Some(selection) = &self.selection {
            app.set_preview_size(format_bytes(selection.selected_bytes(), self.locale).into());
            app.set_preview_can_confirm(selection.can_confirm());
        } else {
            app.set_preview_can_confirm(false);
        }
        app.set_preview_folder(display_path(&self.preview_folder).into());
        if let Some(preview) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.preview.as_ref())
        {
            let path = if preview.multi_file {
                self.preview_folder.join(&preview.name)
            } else {
                preview
                    .files
                    .first()
                    .map(|file| self.preview_folder.join(&file.relative_path))
                    .unwrap_or_else(|| self.preview_folder.clone())
            };
            app.set_preview_final_path(display_path(&path).into());
        }
    }
    fn event(&mut self, app: &MainWindow, event: AppEvent) {
        match event {
            AppEvent::PreviewReady(_) => {
                let _ = app.window().show();
                crate::platform::focus(app.window());
                let _ = self.commands.try_send(AppCommand::SetVisible(true));
            }
            AppEvent::FocusTorrent(id) => {
                app.set_about_open(false);
                self.view.category = super::view_state::Category::All;
                self.view.search.clear();
                app.set_search_query("".into());
                app.set_show_settings(false);
                app.set_show_magnet(false);
                self.select(app, &id.to_string());
                if let Some(index) = (0..self.rows.row_count()).find(|i| {
                    self.rows
                        .row_data(*i)
                        .is_some_and(|row| row.id == id.to_string())
                }) {
                    app.set_scroll_y(-(index as f32) * app.get_torrent_row_height());
                }
                let _ = app.window().show();
                crate::platform::focus(app.window());
                let _ = self.commands.try_send(AppCommand::SetVisible(true));
            }
            AppEvent::Error { error, .. } => {
                self.local_error = None;
                app.set_error(error_message(&error.code, self.locale).into());
            }
            AppEvent::CommandFinished { operation, success } => {
                if self.pending == Some(operation) {
                    self.pending = None;
                    let restore_settings_focus = self.settings_from_form;
                    self.settings_from_form = false;
                    app.set_busy(false);
                    if success && operation == AppOperation::ConfirmAdd {
                        app.set_show_settings(false);
                    }
                    if operation == AppOperation::UpdateSettings
                        && let Some(snapshot) = self.snapshot.clone()
                    {
                        if !success {
                            apply_layout_fields(app, snapshot.settings.ui.layout.normalized());
                        }
                        self.apply(app, snapshot);
                    }
                    if app.get_show_settings() && restore_settings_focus {
                        app.set_settings_focus_ticket(
                            app.get_settings_focus_ticket().wrapping_add(1),
                        );
                    } else if !app.get_show_settings() && operation != AppOperation::UpdateSettings
                    {
                        app.invoke_focus_main();
                    }
                }
                if !success && operation == AppOperation::Shutdown {
                    app.set_stopping(false);
                }
            }
            AppEvent::Completion(_) | AppEvent::ShutdownReady => {}
        }
    }
    fn begin(&mut self, app: &MainWindow) {
        let fatal = self
            .snapshot
            .as_ref()
            .and_then(|s| s.error.as_ref())
            .is_some_and(|e| e.code == ErrorCode::CorruptState);
        self.local_error = if fatal { None } else { Some(String::new()) };
        if !fatal {
            app.set_error("".into());
        }
    }
    fn queue(&mut self, app: &MainWindow, command: AppCommand) {
        if self.pending.is_some() || app.get_confirm_open() {
            return;
        }
        self.begin(app);
        let operation = command.operation();
        if self.commands.try_send(command).is_ok() {
            self.pending = Some(operation);
            app.set_busy(true);
        } else {
            let message = self.locale.text(
                "Приложение занято. Попробуйте ещё раз.",
                "The app is busy. Try again.",
            );
            self.local_error = Some(message.into());
            app.set_error(message.into());
        }
    }
    fn row(&self, id: &str) -> Option<TorrentSnapshot> {
        self.snapshot
            .as_ref()?
            .torrents
            .iter()
            .find(|row| row.id.to_string() == id)
            .cloned()
    }
}

fn sync_locale_copy(app: &MainWindow, locale: Locale) {
    app.set_about_copy(super::about::copy(locale));
    let names = std::iter::once(locale.text("Системный", "System"))
        .chain(Locale::ALL.iter().map(|locale| locale.autonym()))
        .map(slint::SharedString::from)
        .collect::<Vec<_>>();
    app.set_language_names(std::rc::Rc::new(slint::VecModel::from(names)).into());
}
fn sync_category_labels(app: &MainWindow, locale: Locale) {
    use super::{i18n::category_label, view_state::Category};
    let downloading = category_label(Category::Incomplete, locale);
    let completed = category_label(Category::Downloaded, locale);
    if app.get_downloading_label() != downloading {
        app.set_downloading_label(downloading.into());
    }
    if app.get_completed_label() != completed {
        app.set_completed_label(completed.into());
    }
}
fn bind_callbacks(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
    app.global::<Translations>().on_lookup(|index, ru, en| {
        Locale::from_index(index)
            .unwrap_or(Locale::En)
            .text(&ru, &en)
            .into()
    });
    app.global::<Translations>()
        .set_locale(state.borrow().locale.index());
    sync_category_labels(app, state.borrow().locale);
    app.set_app_version(env!("CARGO_PKG_VERSION").into());
    sync_locale_copy(app, state.borrow().locale);
    {
        let weak = app.as_weak();
        app.on_about_requested(move || {
            if let Some(app) = weak.upgrade()
                && !app.get_busy()
                && !app.get_stopping()
                && !app.get_confirm_open()
                && !app.get_preview_open()
                && !app.get_show_magnet()
            {
                app.set_about_error("".into());
                app.set_about_open(true);
            }
        });
    }
    {
        let weak = app.as_weak();
        app.on_close_about(move || {
            if let Some(app) = weak.upgrade() {
                app.set_about_open(false);
                if app.get_show_settings() {
                    app.set_settings_focus_ticket(app.get_settings_focus_ticket().wrapping_add(1));
                } else {
                    app.invoke_focus_main();
                }
            }
        });
    }
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_about_link(move |index| {
            let Some(app) = weak.upgrade() else {
                return;
            };
            if !app.get_about_open() || app.get_about_link_pending() {
                return;
            }
            app.set_about_link_pending(true);
            app.set_about_error("".into());
            let weak = app.as_weak();
            let locale = state.borrow().locale;
            std::thread::spawn(move || {
                let failed = super::about::open_link(index, |url| open::that(url))
                    .is_some_and(|r| r.is_err());
                let _ = weak.upgrade_in_event_loop(move |app| {
                    app.set_about_link_pending(false);
                    if failed {
                        app.set_about_error(super::about::link_error(locale, index).into());
                    }
                });
            });
        });
    }

    macro_rules! bind0 {
        ($callback:ident,$handler:ident) => {{
            let weak = app.as_weak();
            let state = state.clone();
            app.$callback(move || {
                if let Some(app) = weak.upgrade() {
                    $handler(&app, &state);
                }
            });
        }};
    }
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_selection_changed(move |id, toggle, range| {
            if let Some(app) = weak.upgrade() {
                if app.get_confirm_open() {
                    return;
                }
                let mut state = state.borrow_mut();
                if let Some(id) = parse_id(&id) {
                    let order = (0..state.rows.row_count())
                        .filter_map(|i| parse_id(&state.rows.row_data(i)?.id))
                        .collect::<Vec<_>>();
                    state.view.click(id, &order, toggle, range);
                    state.refresh_view(&app);
                }
            }
        });
    }
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_details_requested(move |id| {
            if let Some(app) = weak.upgrade() {
                if app.get_confirm_open() {
                    return;
                }
                let mut state = state.borrow_mut();
                if let Some(id) = parse_id(&id) {
                    let order = (0..state.rows.row_count())
                        .filter_map(|i| parse_id(&state.rows.row_data(i)?.id))
                        .collect::<Vec<_>>();
                    state.view.toggle_details(id, &order);
                    state.refresh_view(&app);
                }
            }
        });
    }
    bind0!(on_layout_changed, save_layout);
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_filter_changed(move |index| {
            if let Some(app) = weak.upgrade() {
                let mut s = state.borrow_mut();
                app.set_show_settings(false);
                s.view.details_open = false;
                s.view.category = match index {
                    1 => super::view_state::Category::Incomplete,
                    2 => super::view_state::Category::Downloaded,
                    _ => super::view_state::Category::All,
                };
                s.refresh_view(&app);
                app.set_scroll_y(0.0);
            }
        });
    }
    for callback in [0, 1, 2, 3, 4] {
        let weak = app.as_weak();
        let state = state.clone();
        let handler = move || {
            if let Some(app) = weak.upgrade() {
                let mut s = state.borrow_mut();
                if callback == 4 && !app.get_confirm_open() && !s.view.selected.is_empty() {
                    s.view.details_open = true;
                }
                if callback == 1 {
                    s.view.details_open = false;
                }
                if callback == 2 {
                    s.view.selecting = false;
                    app.set_search_open(false);
                    app.set_search_query("".into());
                }
                s.refresh_view(&app);
            }
        };
        match callback {
            0 => app.on_view_changed(handler),
            1 => app.on_close_details(handler),
            2 => app.on_finish_selection(handler),
            3 => app.on_files_changed(handler),
            _ => app.on_show_selection_actions(handler),
        }
    }
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_all_action(move |active| {
            if let Some(app) = weak.upgrade() {
                state
                    .borrow_mut()
                    .queue(&app, AppCommand::SetAllActive(active));
            }
        });
    }
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_group_action(move || {
            if let Some(app) = weak.upgrade() {
                let mut s = state.borrow_mut();
                if let Some(id) = s.view.single() {
                    if let Some(row) = s.row(&id.to_string()) {
                        let command = if row.state == TransferState::Error {
                            AppCommand::Retry(id)
                        } else if row.desired_state == DesiredState::Paused {
                            AppCommand::Resume(id)
                        } else {
                            AppCommand::Pause(id)
                        };
                        s.queue(&app, command);
                    }
                } else {
                    let ids = s.view.selected.iter().copied().collect();
                    let active = s.snapshot.as_ref().is_some_and(|snapshot| {
                        snapshot
                            .torrents
                            .iter()
                            .filter(|r| s.view.selected.contains(&r.id))
                            .all(|r| r.desired_state == DesiredState::Paused)
                    });
                    s.queue(&app, AppCommand::SetActive(ids, active));
                }
            }
        });
    }
    bind0!(on_preferences_changed, save_settings);
    bind0!(on_rates_changed, save_settings);
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_toggle_file_group(move |path| {
            if let Some(app) = weak.upgrade() {
                let mut s = state.borrow_mut();
                if !s.collapsed_folders.remove(path.as_str()) {
                    s.collapsed_folders.insert(path.to_string());
                }
                s.refresh_view(&app);
            }
        });
    }
    bind0!(on_open_requested, open_torrent);
    bind0!(on_magnet_requested, open_magnet);
    bind0!(on_submit_magnet, submit_magnet);
    bind0!(on_cancel_magnet, cancel_magnet);
    bind0!(on_settings_requested, open_settings);
    bind0!(on_choose_preview_folder, choose_preview_folder);
    bind0!(on_confirm_preview, confirm_preview);
    bind0!(on_cancel_preview, cancel_preview);
    bind0!(on_choose_settings_folder, choose_settings_folder);
    bind0!(on_save_settings, save_settings);
    bind0!(on_cancel_settings, cancel_settings);
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_select_row(move |id| {
            if let Some(app) = weak.upgrade() {
                state.borrow_mut().select(&app, &id);
            }
        });
    }
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_toggle_preview_file(move |id, checked| {
            if id < 0 {
                return;
            }
            if let Some(app) = weak.upgrade() {
                let mut state = state.borrow_mut();
                if let Some(selection) = &mut state.selection {
                    selection.set_checked(FileId(id as usize), checked);
                }
                if let Some(index) = (0..state.files.row_count()).find(|index| {
                    state
                        .files
                        .row_data(*index)
                        .is_some_and(|file| file.id == id)
                }) && let Some(mut file) = state.files.row_data(index)
                {
                    file.checked = checked;
                    state.files.set_row_data(index, file);
                }
                state.sync_preview(&app);
            }
        });
    }
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_row_action(move |id| {
            if let Some(app) = weak.upgrade() {
                let row = state.borrow().row(&id);
                if let Some(row) = row {
                    let command = if row.state == TransferState::Error {
                        AppCommand::Retry(row.id)
                    } else if row.desired_state == DesiredState::Paused {
                        AppCommand::Resume(row.id)
                    } else {
                        AppCommand::Pause(row.id)
                    };
                    state.borrow_mut().queue(&app, command);
                }
            }
        });
    }
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_open_folder(move |id| {
            let row = state.borrow().row(&id);
            let locale = state.borrow().locale;
            if let Some(row) = row {
                let weak = weak.clone();
                std::thread::spawn(move || {
                    let result = if row.destination.is_dir() {
                        open::that(&row.destination)
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "folder unavailable",
                        ))
                    };
                    if result.is_err() {
                        let message = format!(
                            "{} {}",
                            error_message(&ErrorCode::UnavailableFolder, locale),
                            row.destination.display()
                        );
                        let _ = weak.upgrade_in_event_loop(move |app| {
                            app.invoke_native_error(message.into())
                        });
                    }
                });
            }
        });
    }
    for with_files in [false, true] {
        let weak = app.as_weak();
        let state = state.clone();
        let handler = move |id: slint::SharedString| {
            if let Some(app) = weak.upgrade() {
                request_removal(&app, &state, &id, with_files);
            }
        };
        if with_files {
            app.on_trash_requested(handler);
        } else {
            app.on_remove_requested(handler);
        }
    }
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_cancel_removal(move || {
            if let Some(app) = weak.upgrade() {
                state.borrow_mut().removal = None;
                app.set_confirm_open(false);
                app.invoke_focus_main();
            }
        });
    }
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_confirm_removal(move || {
            if let Some(app) = weak.upgrade() {
                let mut s = state.borrow_mut();
                if let Some((ids, files)) = s.removal.take() {
                    app.set_confirm_open(false);
                    s.queue(&app, AppCommand::RemoveMany(ids, files));
                }
            }
        });
    }
    {
        let weak = app.as_weak();
        app.on_copy_magnet(move |id| {
            if parse_id(&id).is_some()
                && let Some(app) = weak.upgrade()
            {
                app.invoke_copy_link(id);
            }
        });
    }
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_navigate(move |id, step, range| {
            if let Some(app) = weak.upgrade() {
                let mut s = state.borrow_mut();
                let order = (0..s.rows.row_count())
                    .filter_map(|i| parse_id(&s.rows.row_data(i)?.id))
                    .collect::<Vec<_>>();
                if let Some(index) =
                    parse_id(&id).and_then(|id| order.iter().position(|v| *v == id))
                {
                    let next = (index as i64 + i64::from(step))
                        .clamp(0, order.len().saturating_sub(1) as i64)
                        as usize;
                    let id = order[next];
                    s.view.click(id, &order, false, range);
                    app.set_focused_id(id.to_string().into());
                    s.refresh_view(&app);
                }
            }
        });
    }
}
fn request_removal(app: &MainWindow, state: &Rc<RefCell<UiState>>, id: &str, with_files: bool) {
    let mut s = state.borrow_mut();
    if s.pending.is_some() || app.get_confirm_open() {
        return;
    }
    let ids: Vec<_> = if let Some(id) = parse_id(id) {
        vec![id]
    } else {
        s.view.selected.iter().copied().collect()
    };
    let rows = ids
        .iter()
        .filter_map(|id| s.row(&id.to_string()))
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return;
    }
    let names = rows
        .iter()
        .map(|r| r.name.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let total = rows
        .iter()
        .fold(0u64, |n, r| n.saturating_add(r.selected_bytes));
    let consequence = if with_files {
        s.locale.text("Торренты будут остановлены. Выбранные скачанные файлы будут отправлены в системную Корзину. Посторонние файлы в папке останутся.","The torrents will stop. Their selected downloaded files will move to the system Trash. Unrelated files in the folder will stay.")
    } else {
        s.locale.text("Торренты будут остановлены и удалены из списка. Все скачанные файлы останутся на диске.","The torrents will stop and leave the list. All downloaded files will stay on disk.")
    };
    app.set_confirm_title(
        s.locale
            .text(
                if with_files {
                    "В Корзину"
                } else {
                    "Удалить торрент"
                },
                if with_files {
                    "Move to Trash"
                } else {
                    "Remove torrent"
                },
            )
            .into(),
    );
    app.set_confirm_content(
        format!(
            "{names}\n\n{} · {}\n\n{consequence}",
            rows.len(),
            format_bytes(total, s.locale)
        )
        .into(),
    );
    s.removal = Some((rows.iter().map(|r| r.id).collect(), with_files));
    app.set_confirm_open(true);
}
fn open_torrent(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
    if app.get_confirm_open() {
        return;
    }
    let locale = state.borrow().locale;
    state.borrow_mut().begin(app);
    app.set_busy(true);
    let picked = rfd::FileDialog::new()
        .set_parent(&app.window().window_handle())
        .set_title(locale.text("Добавить торрент", "Add torrent"))
        .add_filter(locale.text("Торрент-файлы", "Torrent files"), &["torrent"])
        .pick_files();
    app.set_busy(false);
    if let Some(files) = picked {
        let commands = state.borrow().commands.clone();
        tokio::spawn(async move {
            for file in files {
                if commands.send(AppCommand::OpenTorrent(file)).await.is_err() {
                    break;
                }
            }
        });
    }
}
fn open_magnet(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
    if app.get_confirm_open() {
        return;
    }
    state.borrow_mut().begin(app);
    app.set_magnet_link("".into());
    app.set_show_settings(false);
    app.set_show_magnet(true);
}
fn submit_magnet(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
    let link = app.get_magnet_link();
    match crate::engine::magnet::normalize_magnet(&link) {
        Ok(link) => state.borrow_mut().queue(app, AppCommand::OpenMagnet(link)),
        Err(error) => {
            let mut state = state.borrow_mut();
            let message = error_message(&error.user_error().code, state.locale).to_owned();
            state.local_error = Some(message.clone());
            app.set_error(message.into());
        }
    }
}
fn cancel_magnet(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
    let mut state = state.borrow_mut();
    if state
        .snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.resolving_magnet)
    {
        state.queue(app, AppCommand::CancelPreview);
    } else {
        state.begin(app);
    }
    app.set_show_magnet(false);
    app.set_magnet_link("".into());
}
fn open_settings(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
    if app.get_confirm_open() || state.borrow().pending.is_some() {
        return;
    }
    let mut state = state.borrow_mut();
    state.begin(app);
    let Some(settings) = state.snapshot.as_ref().map(|s| s.settings.clone()) else {
        return;
    };
    sync_settings_fields(app, &mut state, &settings);
    state.view.details_open = false;
    app.set_show_settings(true);
    state.refresh_view(app);
}
fn apply_layout_fields(app: &MainWindow, layout: UiLayout) {
    app.set_sidebar_width(f32::from(layout.sidebar_width));
    app.set_sidebar_collapsed(layout.sidebar_collapsed);
    app.set_drawer_height(f32::from(layout.drawer_height));
}
fn save_layout(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
    let mut state = state.borrow_mut();
    if state.pending.is_some() || app.get_confirm_open() {
        return;
    }
    let Some(snapshot) = state.snapshot.clone() else {
        return;
    };
    let layout = UiLayout {
        sidebar_width: app.get_sidebar_width().round().clamp(158.0, 280.0) as u16,
        sidebar_collapsed: app.get_sidebar_collapsed(),
        drawer_height: app.get_drawer_height().round().clamp(220.0, 540.0) as u16,
    };
    if layout == snapshot.settings.ui.layout.normalized() {
        return;
    }
    let mut settings = snapshot.settings.clone();
    settings.ui.layout = layout;
    state.queue(app, AppCommand::UpdateSettings(settings));
    if state.pending != Some(AppOperation::UpdateSettings) {
        apply_layout_fields(app, snapshot.settings.ui.layout.normalized());
    }
}
fn sync_settings_fields(app: &MainWindow, state: &mut UiState, settings: &Settings) {
    state.settings_folder = settings.download_dir.clone();
    app.set_settings_folder(display_path(&settings.download_dir).into());
    app.set_settings_notify(settings.notify_on_complete);
    app.set_unlimited_download(settings.limits.download_bps.is_none());
    app.set_unlimited_upload(settings.limits.upload_bps.is_none());
    app.set_download_rate(rate_text(settings.limits.download_bps, state.locale).into());
    app.set_upload_rate(rate_text(settings.limits.upload_bps, state.locale).into());
    app.set_settings_theme(theme_index(settings.ui.theme));
    app.set_settings_language(settings.ui.language.choice());
    app.set_settings_login(settings.ui.launch_at_login);
    app.set_download_preset(rate_preset(settings.limits.download_bps));
    app.set_upload_preset(rate_preset(settings.limits.upload_bps));
    update_rates(app, state.locale, settings.limits);
    app.set_speed_expanded(false);
}
fn theme_index(theme: ThemePreference) -> i32 {
    match theme {
        ThemePreference::System => 0,
        ThemePreference::Light => 1,
        ThemePreference::Dark => 2,
    }
}
fn rate_text(rate: Option<std::num::NonZeroU32>, locale: Locale) -> String {
    let value = rate_mbit_text(rate);
    if locale.decimal_comma() {
        value.replace('.', ",")
    } else {
        value
    }
}
const PRESETS: [u32; 8] = [
    62_500, 125_000, 250_000, 625_000, 1_250_000, 2_500_000, 6_250_000, 12_500_000,
];
fn rate_preset(rate: Option<std::num::NonZeroU32>) -> i32 {
    match rate {
        None => 0,
        Some(rate) => PRESETS
            .iter()
            .position(|r| *r == rate.get())
            .map(|i| i as i32 + 1)
            .unwrap_or(9),
    }
}
fn draft_rate(index: i32, text: &str) -> AppResult<Option<std::num::NonZeroU32>> {
    if index == 0 {
        Ok(None)
    } else if let Some(value) = usize::try_from(index - 1).ok().and_then(|i| PRESETS.get(i)) {
        Ok(std::num::NonZeroU32::new(*value))
    } else {
        parse_rate_mbit(text, false)
    }
}
fn update_rates(app: &MainWindow, locale: Locale, old: TransferLimits) {
    let equivalent = |index, text: &str, previous| match preserved_draft_rate(index, text, previous)
    {
        Ok(None) => locale.text("Без ограничений", "Unlimited").to_owned(),
        Ok(Some(rate)) => format!(
            "= {}",
            super::i18n::format_rate_equivalent(rate.get(), locale)
        ),
        Err(_) => locale
            .text(
                "Введите от 0,5 Мбит/с, с точностью до байта/с",
                "Enter at least 0.5 Mbit/s, in whole bytes/s",
            )
            .to_owned(),
    };
    app.set_download_equivalent(
        equivalent(
            app.get_download_preset(),
            &app.get_download_rate(),
            old.download_bps,
        )
        .into(),
    );
    app.set_upload_equivalent(
        equivalent(
            app.get_upload_preset(),
            &app.get_upload_rate(),
            old.upload_bps,
        )
        .into(),
    );
}

fn choose_preview_folder(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
    let folder = state.borrow().preview_folder.clone();
    let locale = state.borrow().locale;
    if let Some(folder) = rfd::FileDialog::new()
        .set_parent(&app.window().window_handle())
        .set_title(locale.text("Папка сохранения", "Download folder"))
        .set_directory(folder)
        .pick_folder()
    {
        let mut state = state.borrow_mut();
        state.begin(app);
        state.preview_folder = folder;
        state.sync_preview(app);
    }
}
fn choose_settings_folder(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
    let folder = state.borrow().settings_folder.clone();
    let locale = state.borrow().locale;
    if let Some(folder) = rfd::FileDialog::new()
        .set_parent(&app.window().window_handle())
        .set_title(locale.text("Папка по умолчанию", "Default download folder"))
        .set_directory(folder)
        .pick_folder()
    {
        state.borrow_mut().settings_folder = folder.clone();
        app.set_settings_folder(display_path(&folder).into());
        save_settings(app, state);
    }
}
fn confirm_preview(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
    let request = {
        let state = state.borrow();
        match (state.preview_id, state.selection.as_ref()) {
            (Some(id), Some(selection)) if selection.can_confirm() => Some(AddRequest::new(
                id,
                state.preview_folder.clone(),
                selection.selected_files(),
            )),
            _ => None,
        }
    };
    if let Some(request) = request {
        let mut state = state.borrow_mut();
        state.auto_select = Some(request.id);
        state.queue(app, AppCommand::ConfirmAdd(request));
    }
}
fn cancel_preview(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
    state.borrow_mut().queue(app, AppCommand::CancelPreview);
}
fn preserved_draft_rate(
    index: i32,
    text: &str,
    old: Option<std::num::NonZeroU32>,
) -> AppResult<Option<std::num::NonZeroU32>> {
    if index == 9 && old.is_some() && text.trim().replace(',', ".") == rate_mbit_text(old) {
        Ok(old)
    } else {
        draft_rate(index, text)
    }
}
fn save_settings(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
    if !app.get_show_settings() || state.borrow().pending.is_some() || app.get_confirm_open() {
        return;
    }
    let result = (|| -> AppResult<Settings> {
        Ok(Settings {
            ui: crate::settings::UiPreferences {
                theme: match app.get_settings_theme() {
                    1 => ThemePreference::Light,
                    2 => ThemePreference::Dark,
                    _ => ThemePreference::System,
                },
                language: LanguagePreference::from_choice(app.get_settings_language()).ok_or_else(
                    || crate::error::AppError::Engine(anyhow::anyhow!("Invalid language choice")),
                )?,
                launch_at_login: app.get_settings_login(),
                layout: state
                    .borrow()
                    .snapshot
                    .as_ref()
                    .map(|s| s.settings.ui.layout.normalized())
                    .unwrap_or_default(),
            },
            download_dir: state.borrow().settings_folder.clone(),
            notify_on_complete: app.get_settings_notify(),
            limits: TransferLimits {
                download_bps: preserved_draft_rate(
                    app.get_download_preset(),
                    &app.get_download_rate(),
                    state
                        .borrow()
                        .snapshot
                        .as_ref()
                        .and_then(|s| s.settings.limits.download_bps),
                )?,
                upload_bps: preserved_draft_rate(
                    app.get_upload_preset(),
                    &app.get_upload_rate(),
                    state
                        .borrow()
                        .snapshot
                        .as_ref()
                        .and_then(|s| s.settings.limits.upload_bps),
                )?,
            },
        })
    })();
    match result {
        Ok(settings) => {
            let mut state = state.borrow_mut();
            if state
                .snapshot
                .as_ref()
                .is_some_and(|s| s.settings == settings)
            {
                return;
            }
            state.queue(app, AppCommand::UpdateSettings(settings));
            state.settings_from_form = state.pending == Some(AppOperation::UpdateSettings);
            if state.pending != Some(AppOperation::UpdateSettings)
                && let Some(snapshot) = state.snapshot.clone()
            {
                sync_settings_fields(app, &mut state, &snapshot.settings);
            }
        }
        Err(error) => {
            let mut state = state.borrow_mut();
            let message = error_message(&error.user_error().code, state.locale);
            state.local_error = Some(message.into());
            app.set_error(message.into());
        }
    }
}
fn cancel_settings(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
    let mut state = state.borrow_mut();
    state.begin(app);
    app.set_show_settings(false);
    app.invoke_focus_main();
    if let Some(snapshot) = state.snapshot.clone() {
        state.apply(app, snapshot);
    }
}

#[cfg(test)]
mod magnet_focus_tests {
    use super::*;
    use slint::platform::{
        Platform, PlatformError, WindowAdapter,
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    };

    struct Headless;
    impl Platform for Headless {
        fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
            Ok(MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer))
        }
    }

    #[test]
    fn duplicate_magnet_focus_reveals_existing_row_and_preserves_next_preview() {
        slint::platform::set_platform(Box::new(Headless)).unwrap();
        let app = MainWindow::new().unwrap();
        let id = TorrentId([4; 20]);
        let second = TorrentId([5; 20]);
        let rows = Rc::new(slint::VecModel::from(vec![
            TorrentRow {
                id: id.to_string().into(),
                name: "Existing torrent".into(),
                ..Default::default()
            },
            TorrentRow {
                id: second.to_string().into(),
                name: "Second".into(),
                ..Default::default()
            },
        ]));
        let (commands, mut requests) = mpsc::channel(32);
        let mut state = UiState {
            locale: Locale::En,
            system_locale: Locale::En,
            notification_sink: Arc::new(NoopNotificationSink),
            shell: None,
            view: Default::default(),
            detail_files: Rc::new(slint::VecModel::default()),
            detail_requested: None,
            removal: None,
            collapsed_folders: Default::default(),
            commands,
            snapshot: None,
            rows,
            files: Rc::new(slint::VecModel::default()),
            preview_id: None,
            selection: None,
            preview_folder: PathBuf::new(),
            settings_folder: PathBuf::new(),
            last_layout: None,
            pending: None,
            settings_from_form: false,
            auto_select: None,
            local_error: None,
        };
        app.set_show_settings(true);
        app.set_show_magnet(true);
        state.event(&app, AppEvent::FocusTorrent(id));
        assert!(
            !app.get_show_magnet(),
            "duplicate remains hidden behind magnet input"
        );
        assert!(!app.get_show_settings());
        assert_eq!(app.get_selected_row().id, id.to_string());
        app.set_preview_open(true);
        app.set_show_magnet(true);
        state.event(&app, AppEvent::FocusTorrent(id));
        assert!(
            app.get_preview_open(),
            "focus discarded a subsequent queued preview"
        );
        assert!(!app.get_show_magnet());
        app.set_torrent_row_height(132.0);
        state.event(&app, AppEvent::FocusTorrent(second));
        assert_eq!(
            app.get_scroll_y(),
            -132.0,
            "focus scrolling ignored the Slint row geometry"
        );
        while requests.try_recv().is_ok() {}
        let folder = tempfile::tempdir().unwrap();
        let mut settings = Settings::defaults(folder.path().to_owned());
        settings.limits.download_bps = std::num::NonZeroU32::new(20_000);
        let torrent = |id, name: &str| TorrentSnapshot {
            id,
            name: name.into(),
            selected_bytes: 100,
            checked_downloaded_bytes: 50,
            download_bps: 0,
            upload_bps: 0,
            session_uploaded_bytes: 0,
            state: TransferState::Paused,
            error: None,
            destination: folder.path().into(),
            desired_state: DesiredState::Paused,
        };
        state.snapshot = Some(Arc::new(AppSnapshot {
            torrents: vec![torrent(id, "First"), torrent(second, "Second")],
            details: None,
            settings: settings.clone(),
            settings_recovery: None,
            blocked_state_path: None,
            visible: true,
            preview: None,
            resolving_magnet: false,
            error: None,
        }));
        let state = Rc::new(RefCell::new(state));
        bind_callbacks(&app, &state);
        app.set_preview_open(false);
        app.set_show_magnet(false);
        app.invoke_close_details();
        while requests.try_recv().is_ok() {}
        let selected_before = state.borrow().view.selected.clone();
        app.invoke_about_requested();
        assert!(app.get_about_open());
        assert!(app.get_app_version().contains(env!("CARGO_PKG_VERSION")));
        app.invoke_close_about();
        assert!(!app.get_about_open());
        assert_eq!(state.borrow().view.selected, selected_before);
        assert!(
            requests.try_recv().is_err(),
            "About changed engine or saved preferences"
        );
        let original_locale_snapshot = state.borrow().snapshot.as_ref().unwrap().as_ref().clone();
        let mut translated = original_locale_snapshot.clone();
        translated.settings.ui.language = LanguagePreference::Ja;
        state
            .borrow_mut()
            .apply(&app, std::sync::Arc::new(translated));
        assert_eq!(app.get_all_label(), "すべて");
        assert!(app.get_about_copy().title.contains("FlowTorrent"));
        assert_ne!(app.get_about_copy().title, "About FlowTorrent");
        state
            .borrow_mut()
            .apply(&app, std::sync::Arc::new(original_locale_snapshot));
        assert_eq!(app.get_all_label(), "All");
        app.set_preview_open(true);
        app.invoke_about_requested();
        let about_in_preview = app.get_about_open();
        app.invoke_close_about();
        app.set_preview_open(false);
        app.set_show_magnet(true);
        app.invoke_about_requested();
        let about_in_magnet = app.get_about_open();
        app.invoke_close_about();
        app.set_show_magnet(false);
        let original = state.borrow().snapshot.as_ref().unwrap().as_ref().clone();
        let mut incoming = original.clone();
        incoming.preview = Some(Arc::new(TorrentPreview {
            id: TorrentId([8; 20]),
            name: "Incoming".into(),
            files: vec![TorrentFile {
                id: FileId(0),
                relative_path: "incoming.bin".into(),
                length: 100,
                is_padding: false,
            }],
            metadata: vec![],
            multi_file: false,
        }));
        app.invoke_about_requested();
        state.borrow_mut().apply(&app, Arc::new(incoming));
        let closes_for_preview = !app.get_about_open() && app.get_preview_open();
        state.borrow_mut().apply(&app, Arc::new(original.clone()));
        let mut resolving = original.clone();
        resolving.resolving_magnet = true;
        app.invoke_about_requested();
        state.borrow_mut().apply(&app, Arc::new(resolving));
        let closes_for_resolve = !app.get_about_open() && app.get_show_magnet();
        state.borrow_mut().apply(&app, Arc::new(original));
        app.set_show_magnet(false);
        app.invoke_about_requested();
        state.borrow_mut().event(&app, AppEvent::FocusTorrent(id));
        let closes_for_focus = !app.get_about_open();
        app.invoke_close_details();
        while requests.try_recv().is_ok() {}
        assert_eq!(
            (
                about_in_preview,
                about_in_magnet,
                closes_for_preview,
                closes_for_resolve,
                closes_for_focus
            ),
            (false, false, true, true, true),
            "About interfered with foreground add/focus handling"
        );
        app.invoke_selection_changed(id.to_string().into(), false, false);
        assert!(!app.get_details_open());
        assert!(
            !std::iter::from_fn(|| requests.try_recv().ok())
                .any(|c| matches!(c, AppCommand::InspectDetails(Some(_))))
        );
        app.invoke_details_requested(id.to_string().into());
        assert!(app.get_details_open());
        assert!(
            std::iter::from_fn(|| requests.try_recv().ok())
                .any(|c| matches!(c,AppCommand::InspectDetails(Some(actual)) if actual==id))
        );
        app.invoke_close_details();
        assert!(!app.get_details_open());
        assert_eq!(app.get_selected_row().id, id.to_string());
        assert!(
            std::iter::from_fn(|| requests.try_recv().ok())
                .any(|c| matches!(c, AppCommand::InspectDetails(None)))
        );
        app.invoke_selection_changed(second.to_string().into(), true, false);
        app.invoke_show_selection_actions();
        assert!(app.get_details_open());
        assert_eq!(app.get_selected_count(), 2);
        assert_eq!(state.borrow().view.selected.len(), 2);
        assert!(state.borrow().view.inspected().is_none());
        assert!(
            !std::iter::from_fn(|| requests.try_recv().ok())
                .any(|c| matches!(c, AppCommand::InspectDetails(Some(_))))
        );
        app.invoke_close_details();
        app.invoke_finish_selection();
        app.invoke_selection_changed(id.to_string().into(), false, false);
        assert!(app.get_selected_row().metrics_known);
        assert_eq!(app.get_selected_row().size, "100 B");
        assert_eq!(app.get_selected_row().downloaded, "50 B");
        assert!(!app.get_checkboxes());
        assert_eq!(app.get_selected_row().id, id.to_string());
        app.set_search_open(true);
        app.invoke_view_changed();
        assert!(app.get_checkboxes());
        app.invoke_finish_selection();
        assert!(!app.get_checkboxes());
        while requests.try_recv().is_ok() {}
        app.invoke_all_action(true);
        assert!(matches!(
            requests.try_recv(),
            Ok(AppCommand::SetAllActive(true))
        ));
        state.borrow_mut().pending = None;
        app.set_busy(false);
        request_removal(&app, &state, &id.to_string(), false);
        app.invoke_selection_changed(second.to_string().into(), false, false);
        app.invoke_confirm_removal();
        let command = std::iter::from_fn(|| requests.try_recv().ok())
            .find(|c| matches!(c, AppCommand::RemoveMany(_, _)))
            .unwrap();
        assert!(
            matches!(command,AppCommand::RemoveMany(ids,false) if ids==vec![id]),
            "confirmation followed a later selection"
        );
        state.borrow_mut().pending = None;
        app.set_busy(false);
        while requests.try_recv().is_ok() {}
        open_settings(&app, &state);
        assert!(
            !std::iter::from_fn(|| requests.try_recv().ok())
                .any(|c| matches!(c, AppCommand::UpdateSettings(_))),
            "opening settings queued a write"
        );
        assert_eq!(app.get_download_preset(), 9);
        assert_eq!(app.get_download_rate(), "0.16");
        assert!(app.get_download_equivalent().contains("20 KB/s"));
        app.set_settings_notify(false);
        app.invoke_preferences_changed();
        assert_eq!(
            state.borrow().pending,
            Some(AppOperation::UpdateSettings),
            "a preference change still requires Save"
        );
        let submitted = std::iter::from_fn(|| requests.try_recv().ok())
            .find_map(|c| match c {
                AppCommand::UpdateSettings(s) => Some(s),
                _ => None,
            })
            .unwrap();
        assert!(!submitted.notify_on_complete);
        assert_eq!(
            submitted.ui.language, settings.ui.language,
            "unrelated edit replaced System language"
        );
        assert_eq!(
            submitted.limits.download_bps, settings.limits.download_bps,
            "unrelated edit changed a legacy limit"
        );
        let mut saved = state.borrow().snapshot.as_ref().unwrap().as_ref().clone();
        saved.settings = submitted;
        state.borrow_mut().apply(&app, Arc::new(saved));
        state.borrow_mut().event(
            &app,
            AppEvent::CommandFinished {
                operation: AppOperation::UpdateSettings,
                success: true,
            },
        );
        assert!(app.get_show_settings(), "automatic save closed Settings");
        assert_eq!(
            app.get_settings_focus_ticket(),
            1,
            "automatic save did not restore the initiating control"
        );
        assert!(!app.get_busy());
        app.set_settings_language(LanguagePreference::Ru.choice());
        app.set_settings_theme(2);
        app.invoke_preferences_changed();
        assert!(!app.get_ru(), "uncommitted appearance was applied");
        state.borrow_mut().event(
            &app,
            AppEvent::CommandFinished {
                operation: AppOperation::UpdateSettings,
                success: false,
            },
        );
        assert!(app.get_show_settings());
        assert_eq!(
            app.get_settings_language(),
            LanguagePreference::System.choice()
        );
        assert_eq!(app.get_settings_theme(), 0);
        assert_eq!(
            app.get_settings_focus_ticket(),
            2,
            "failed save did not restore settings focus"
        );
        assert!(!app.get_ru());
        while requests.try_recv().is_ok() {}
        app.set_download_rate("0.49".into());
        save_settings(&app, &state);
        assert!(state.borrow().pending.is_none());
        assert!(requests.try_recv().is_err());
        cancel_settings(&app, &state);
        assert!(!app.get_ru());
        assert_eq!(
            state.borrow().snapshot.as_ref().unwrap().settings.limits,
            settings.limits
        );
        while requests.try_recv().is_ok() {}
        app.set_sidebar_width(200.0);
        app.set_sidebar_collapsed(true);
        app.set_drawer_height(420.0);
        app.invoke_layout_changed();
        let layout_settings = std::iter::from_fn(|| requests.try_recv().ok())
            .find_map(|c| match c {
                AppCommand::UpdateSettings(s) => Some(s),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            layout_settings.ui.layout,
            UiLayout {
                sidebar_width: 200,
                sidebar_collapsed: true,
                drawer_height: 420
            }
        );
        assert_eq!(layout_settings.limits, settings.limits);
        assert!(!layout_settings.ui.launch_at_login);
        let mut layout_snapshot = state.borrow().snapshot.as_ref().unwrap().as_ref().clone();
        layout_snapshot.settings = layout_settings;
        state.borrow_mut().apply(&app, Arc::new(layout_snapshot));
        state.borrow_mut().event(
            &app,
            AppEvent::CommandFinished {
                operation: AppOperation::UpdateSettings,
                success: true,
            },
        );
        app.set_sidebar_width(220.0);
        let same_snapshot = state.borrow().snapshot.clone().unwrap();
        state.borrow_mut().apply(&app, same_snapshot);
        assert_eq!(
            app.get_sidebar_width(),
            220.0,
            "transfer snapshots reset a resize in progress"
        );
        app.invoke_layout_changed();
        state.borrow_mut().event(
            &app,
            AppEvent::CommandFinished {
                operation: AppOperation::UpdateSettings,
                success: false,
            },
        );
        assert_eq!(
            app.get_sidebar_width(),
            200.0,
            "failed layout save was not rolled back"
        );
        state.borrow_mut().view.category = super::super::view_state::Category::Downloaded;
        app.set_search_query("no match".into());
        state.borrow_mut().event(&app, AppEvent::FocusTorrent(id));
        assert_eq!(
            app.get_selected_row().id,
            id.to_string(),
            "duplicate focus must reveal a filtered-out torrent"
        );
        use slint::winit_030::winit::{event::WindowEvent, window::Theme};
        app.invoke_apply_theme(1);
        native_theme_event(&app, &WindowEvent::ThemeChanged(Theme::Dark));
        assert!(
            !app.get_dark_theme(),
            "system change overrode explicit Light"
        );
        app.invoke_apply_theme(0);
        assert!(
            app.get_dark_theme(),
            "System ignored the latest native appearance"
        );
        native_theme_event(&app, &WindowEvent::ThemeChanged(Theme::Light));
        assert!(!app.get_dark_theme());
        app.invoke_apply_theme(2);
        native_theme_event(&app, &WindowEvent::ThemeChanged(Theme::Light));
        assert!(app.get_dark_theme(), "system change overrode explicit Dark");
        let mut failed = state.borrow().snapshot.as_ref().unwrap().as_ref().clone();
        failed.torrents[0].state = TransferState::Error;
        failed.torrents[0].selected_bytes = 0;
        failed.torrents[0].checked_downloaded_bytes = 0;
        state.borrow_mut().snapshot = Some(Arc::new(failed));
        app.invoke_view_changed();
        let failed = app.get_selected_row();
        assert!(!failed.metrics_known);
        assert_eq!(failed.size, "—", "unknown size was presented as zero");
        assert_eq!(failed.downloaded, "—");
        assert_eq!(
            failed.eta, "ETA —",
            "restore error was presented as complete"
        );
    }
}
