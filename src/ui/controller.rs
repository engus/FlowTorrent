use super::{
    components::{MainWindow, PreviewFileRow, TorrentRow},
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
    rate::parse_rate_mbps,
    settings::Settings,
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
    commands: mpsc::Sender<AppCommand>,
    snapshot: Option<Arc<AppSnapshot>>,
    rows: Rc<slint::VecModel<TorrentRow>>,
    files: Rc<slint::VecModel<PreviewFileRow>>,
    preview_id: Option<TorrentId>,
    selection: Option<PreviewSelection>,
    preview_folder: PathBuf,
    settings_folder: PathBuf,
    draft_open: bool,
    pending: Option<AppOperation>,
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
        let notifications = match NotificationDispatcher::new(sink) {
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
        let rows = Rc::new(slint::VecModel::default());
        let files = Rc::new(slint::VecModel::default());
        component.set_rows(rows.clone().into());
        component.set_preview_files(files.clone().into());
        let commands = handle.commands.clone();
        let state = Rc::new(RefCell::new(UiState {
            locale,
            commands: commands.clone(),
            snapshot: None,
            rows,
            files,
            preview_id: None,
            selection: None,
            preview_folder: PathBuf::new(),
            settings_folder: PathBuf::new(),
            draft_open: false,
            pending: None,
            auto_select: None,
            local_error: None,
        }));
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
                    (inbox.latest.clone(), std::mem::take(&mut inbox.events))
                };
                scheduled.store(false, Ordering::Release);
                if let Some(snapshot) = snapshot {
                    state.borrow_mut().apply(&app, snapshot);
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
        let pump=runtime.spawn(async move {
            let mut snapshots=handle.snapshots;let mut events=handle.events;
            if snapshots.has_changed().unwrap_or(false){
                let snapshot=Arc::new(snapshots.borrow_and_update().clone());inbox.lock().unwrap().latest=Some(snapshot);
                schedule(&weak,&scheduled);
            }
            loop {
                let force=tokio::select! {
                    result=snapshots.changed()=>{
                        if result.is_err(){quit_ui(&weak);break;}
                        let snapshot=Arc::new(snapshots.borrow_and_update().clone());let visible=snapshot.visible;
                        inbox.lock().unwrap().latest=Some(snapshot);visible
                    }
                    event=events.recv()=>{
                        match event {
                            Some(AppEvent::ShutdownReady)|None=>{quit_ui(&weak);break;}
                            Some(AppEvent::Completion(notice))=>{if let Some(dispatcher) = &notifications { dispatcher.completed(notice); }false},
                            Some(event)=>{
                                let snapshot=Arc::new(snapshots.borrow_and_update().clone());let mut inbox=inbox.lock().unwrap();inbox.latest=Some(snapshot);inbox.events.push(event);true
                            }
                        }
                    }
                };
                if force{schedule(&weak,&scheduled);}
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
                    rfd::MessageDialog::new().set_parent(&app.window().window_handle()).set_title("FlowTorrent").set_description(locale.text(
                        if cfg!(target_os="macos") { "Загрузки продолжатся в фоне. Открыть FlowTorrent можно через Dock или значок в строке меню. Чтобы завершить работу, выберите «Выйти» или нажмите Cmd+Q." } else { "Загрузки продолжатся в фоне. Чтобы вернуть окно, нажмите значок FlowTorrent в трее. Для завершения работы выберите «Выйти» в его меню." },
                        if cfg!(target_os="macos") { "Downloads will continue in the background. Reopen FlowTorrent from the Dock or menu bar icon. Choose Quit or press Cmd+Q to exit." } else { "Downloads will continue in the background. Click the FlowTorrent tray icon to reopen the window. Choose Quit in its menu to exit." }
                    )).show();
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
                                app.invoke_synchronize();
                                let _ = app.show();
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
    }
}
impl UiState {
    fn apply(&mut self, app: &MainWindow, snapshot: Arc<AppSnapshot>) {
        let next: Vec<_> = snapshot
            .torrents
            .iter()
            .map(|row| ui_row(to_row(row, self.locale)))
            .collect();
        let same_order = self.rows.row_count() == next.len()
            && next.iter().enumerate().all(|(index, row)| {
                self.rows
                    .row_data(index)
                    .is_some_and(|old| old.id == row.id)
            });
        if same_order {
            for (index, row) in next.into_iter().enumerate() {
                if self.rows.row_data(index).as_ref() != Some(&row) {
                    self.rows.set_row_data(index, row);
                }
            }
        } else {
            self.rows.set_vec(next);
        }
        let preview_id = snapshot.preview.as_ref().map(|p| p.id);
        if preview_id != self.preview_id {
            self.preview_id = preview_id;
            self.selection = snapshot
                .preview
                .as_ref()
                .map(|preview| PreviewSelection::new(preview));
            self.preview_folder = snapshot.settings.download_dir.clone();
            app.set_files_expanded(false);
            if let Some(preview) = &snapshot.preview {
                app.set_preview_name(preview.name.clone().into());
                self.files.set_vec(
                    self.selection
                        .as_ref()
                        .unwrap()
                        .files()
                        .iter()
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
        self.snapshot = Some(snapshot.clone());
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
        let selected = self
            .auto_select
            .filter(|id| snapshot.torrents.iter().any(|row| row.id == *id))
            .map(|id| id.to_string())
            .unwrap_or_else(|| app.get_selected_row().id.to_string());
        self.select(app, &selected);
        if self
            .auto_select
            .is_some_and(|id| snapshot.torrents.iter().any(|row| row.id == id))
        {
            self.auto_select = None;
            app.set_scroll_y(0.0);
        }
        let error = self.local_error.clone().unwrap_or_else(|| {
            snapshot
                .error
                .as_ref()
                .map(|e| error_message(&e.code, self.locale).to_owned())
                .unwrap_or_default()
        });
        app.set_error(error.into());
    }
    fn select(&self, app: &MainWindow, id: &str) {
        let selected = (0..self.rows.row_count())
            .filter_map(|i| self.rows.row_data(i))
            .find(|row| row.id == id)
            .or_else(|| self.rows.row_data(0))
            .unwrap_or_default();
        app.set_selected_row(selected);
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
                app.set_show_settings(false);
                app.set_show_magnet(false);
                self.select(app, &id.to_string());
                if let Some(index) = (0..self.rows.row_count()).find(|i| {
                    self.rows
                        .row_data(*i)
                        .is_some_and(|row| row.id == id.to_string())
                }) {
                    app.set_scroll_y(-(index as f32) * 108.0);
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
                    app.set_busy(false);
                    if success
                        && matches!(
                            operation,
                            AppOperation::ConfirmAdd | AppOperation::UpdateSettings
                        )
                    {
                        app.set_show_settings(false);
                    }
                    if success && operation == AppOperation::UpdateSettings {
                        self.draft_open = false;
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
        if self.pending.is_some() {
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

fn bind_callbacks(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
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
                state.borrow().select(&app, &id);
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
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_remove_requested(move |id| {
            let Some(app) = weak.upgrade() else {
                return;
            };
            let row = state.borrow().row(&id);
            let locale = state.borrow().locale;
            let Some(row) = row else {
                return;
            };
            let confirm = locale.text("Убрать из списка", "Remove from list");
            let result = rfd::MessageDialog::new()
                .set_parent(&app.window().window_handle())
                .set_title(confirm)
                .set_description(format!(
                    "{}\n\n{}",
                    row.name,
                    locale.text(
                        "Торрент будет остановлен. Скачанные файлы останутся на диске.",
                        "The torrent will stop. Downloaded files will stay on disk."
                    )
                ))
                .set_buttons(rfd::MessageButtons::OkCancelCustom(
                    confirm.into(),
                    locale.text("Отмена", "Cancel").into(),
                ))
                .show();
            if result == rfd::MessageDialogResult::Ok
                || result == rfd::MessageDialogResult::Custom(confirm.into())
            {
                state.borrow_mut().queue(&app, AppCommand::Forget(row.id));
            }
        });
    }
}
fn open_torrent(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
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
    let mut state = state.borrow_mut();
    state.begin(app);
    if !state.draft_open {
        let Some(settings) = state
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.settings.clone())
        else {
            return;
        };
        state.settings_folder = settings.download_dir.clone();
        app.set_settings_folder(display_path(&settings.download_dir).into());
        app.set_settings_notify(settings.notify_on_complete);
        app.set_unlimited_download(settings.limits.download_bps.is_none());
        app.set_unlimited_upload(settings.limits.upload_bps.is_none());
        app.set_download_rate(rate_text(settings.limits.download_bps, state.locale).into());
        app.set_upload_rate(rate_text(settings.limits.upload_bps, state.locale).into());
        app.set_speed_expanded(false);
        state.draft_open = true;
    }
    app.set_show_settings(true);
}
fn rate_text(rate: Option<std::num::NonZeroU32>, locale: Locale) -> String {
    let Some(rate) = rate else {
        return "1".into();
    };
    let rate = rate.get();
    let text = format!("{}.{:06}", rate / 1_000_000, rate % 1_000_000);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if locale == Locale::Ru {
        text.replace('.', ",")
    } else {
        text.into()
    }
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
fn save_settings(app: &MainWindow, state: &Rc<RefCell<UiState>>) {
    let result = (|| -> AppResult<Settings> {
        Ok(Settings {
            download_dir: state.borrow().settings_folder.clone(),
            notify_on_complete: app.get_settings_notify(),
            limits: TransferLimits {
                download_bps: parse_rate_mbps(
                    &app.get_download_rate(),
                    app.get_unlimited_download(),
                )?,
                upload_bps: parse_rate_mbps(&app.get_upload_rate(), app.get_unlimited_upload())?,
            },
        })
    })();
    match result {
        Ok(settings) => state
            .borrow_mut()
            .queue(app, AppCommand::UpdateSettings(settings)),
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
    state.draft_open = false;
    state.begin(app);
    app.set_show_settings(false);
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
        let rows = Rc::new(slint::VecModel::from(vec![TorrentRow {
            id: id.to_string().into(),
            name: "Existing torrent".into(),
            ..Default::default()
        }]));
        let (commands, _requests) = mpsc::channel(8);
        let mut state = UiState {
            locale: Locale::En,
            commands,
            snapshot: None,
            rows,
            files: Rc::new(slint::VecModel::default()),
            preview_id: None,
            selection: None,
            preview_folder: PathBuf::new(),
            settings_folder: PathBuf::new(),
            draft_open: false,
            pending: None,
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
    }
}
