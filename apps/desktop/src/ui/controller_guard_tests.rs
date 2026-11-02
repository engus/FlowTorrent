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
struct Fixture {
    app: MainWindow,
    state: Rc<RefCell<UiState>>,
    requests: mpsc::Receiver<AppCommand>,
    id: TorrentId,
}
impl Fixture {
    fn new() -> Self {
        slint::platform::set_platform(Box::new(Headless)).unwrap();
        let app = MainWindow::new().unwrap();
        let id = TorrentId([7; 20]);
        let (commands, requests) = mpsc::channel(16);
        let mut state = UiState {
            locale: Locale::En,
            system_locale: Locale::En,
            notification_sink: Arc::new(NoopNotificationSink),
            shell: None,
            add_window_parent: None,
            view: Default::default(),
            detail_files: Rc::new(slint::VecModel::default()),
            detail_requested: Some(id),
            removal: None,
            collapsed_folders: Default::default(),
            commands,
            snapshot: None,
            rows: Rc::new(slint::VecModel::default()),
            files: Rc::new(slint::VecModel::default()),
            preview_id: None,
            preview_revision: 0,
            selection: None,
            preview_folder: PathBuf::new(),
            settings_folder: PathBuf::new(),
            last_layout: None,
            pending: None,
            settings_from_form: false,
            auto_select: None,
            local_error: None,
        };
        app.set_rows(state.rows.clone().into());
        app.set_detail_files(state.detail_files.clone().into());
        state.view.toggle_details(id, &[id]);
        state.apply(
            &app,
            Arc::new(AppSnapshot {
                torrents: vec![TorrentSnapshot {
                    priority: FilePriority::Normal,
                    id,
                    name: "bundle".into(),
                    selected_bytes: 100,
                    checked_downloaded_bytes: 0,
                    checking: None,
                    download_bps: 0,
                    upload_bps: 0,
                    session_uploaded_bytes: 0,
                    state: TransferState::Paused,
                    error: None,
                    destination: "downloads".into(),
                    desired_state: DesiredState::Paused,
                }],
                details: Some(Arc::new(TorrentDetails {
                    id,
                    files: vec![FileProgress {
                        id: FileId(3),
                        path: "nested/actual.bin".into(),
                        length: 100,
                        checked_bytes: 0,
                        selected: true,
                        priority: FilePriority::Normal,
                    }],
                })),
                settings: Settings::defaults("downloads".into()),
                settings_recovery: None,
                blocked_state_path: None,
                visible: true,
                preview: None,
                resolving_magnet: false,
                error: None,
            }),
        );
        let state = Rc::new(RefCell::new(state));
        bind_callbacks(&app, &state);
        Self {
            app,
            state,
            requests,
            id,
        }
    }
    fn assert_empty(&mut self, reason: &str) {
        assert!(
            matches!(
                self.requests.try_recv(),
                Err(mpsc::error::TryRecvError::Empty)
            ),
            "transfer command escaped {reason}"
        );
    }
    fn assert_allowed(&mut self, edit: Edit) {
        edit.invoke(&self.app, self.id);
        let command = self
            .requests
            .try_recv()
            .expect("unblocked callback stopped working");
        match (edit, &command) {
            (Edit::Selection, AppCommand::SetFileEnabled(id, FileId(3), false))
            | (
                Edit::FilePriority,
                AppCommand::SetFilePriority(id, FileId(3), FilePriority::High),
            )
            | (Edit::TorrentPriority, AppCommand::SetTorrentPriority(id, FilePriority::High)) => {
                assert_eq!(*id, self.id)
            }
            other => panic!("wrong command/target: {other:?}"),
        }
        self.assert_empty("duplicate dispatch");
        self.state.borrow_mut().event(
            &self.app,
            AppEvent::CommandFinished {
                operation: command.operation(),
                success: false,
            },
        );
        assert!(self.state.borrow().pending.is_none());
        assert!(!self.app.get_busy());
    }
}
#[derive(Clone, Copy, Debug)]
enum Edit {
    Selection,
    FilePriority,
    TorrentPriority,
}
impl Edit {
    const ALL: [Self; 3] = [Self::Selection, Self::FilePriority, Self::TorrentPriority];
    fn invoke(self, app: &MainWindow, id: TorrentId) {
        let id = id.to_string().into();
        match self {
            Self::Selection => app.invoke_file_enabled(id, 3, false),
            Self::FilePriority => app.invoke_file_priority(id, 3, 2),
            Self::TorrentPriority => app.invoke_torrent_priority(id, 2),
        }
    }
}

#[test]
fn transfer_callbacks_reject_busy_and_stopping_without_pending_command() {
    let mut f = Fixture::new();
    for edit in Edit::ALL {
        for stopping in [false, true] {
            assert!(f.state.borrow().pending.is_none());
            f.app.set_busy(!stopping);
            f.app.set_stopping(stopping);
            f.app.set_error("keep this error".into());
            edit.invoke(&f.app, f.id);
            f.assert_empty(if stopping {
                "shutdown"
            } else {
                "native chooser busy state"
            });
            assert!(f.state.borrow().pending.is_none());
            assert_eq!(f.app.get_error(), "keep this error");
            assert_eq!(f.app.get_busy(), !stopping);
            assert_eq!(f.app.get_stopping(), stopping);
            f.app.set_busy(false);
            f.app.set_stopping(false);
            f.assert_allowed(edit);
        }
    }
}

#[test]
fn transfer_callbacks_reject_about_and_work_again_after_closing_it() {
    let mut f = Fixture::new();
    for edit in Edit::ALL {
        f.app.invoke_about_requested();
        assert!(f.app.get_about_open());
        assert!(!f.app.get_show_settings());
        edit.invoke(&f.app, f.id);
        f.assert_empty("About");
        assert!(f.app.get_about_open());
        assert!(f.state.borrow().pending.is_none());
        f.app.invoke_close_about();
        f.assert_allowed(edit);
    }
}

#[test]
fn transfer_callbacks_reject_other_dialogs_and_pending_even_if_busy_flag_is_stale() {
    let mut f = Fixture::new();
    for edit in Edit::ALL {
        for gate in 0..7 {
            match gate {
                0 => f.app.set_preview_open(true),
                1 => f.app.set_show_magnet(true),
                2 => f.app.set_show_settings(true),
                3 => f.app.set_confirm_open(true),
                4 => f.state.borrow_mut().pending = Some(AppOperation::SetFileEnabled),
                5 => f.app.set_resolving_magnet(true),
                _ => f.app.set_ready(false),
            }
            let pending = f.state.borrow().pending;
            edit.invoke(&f.app, f.id);
            f.assert_empty(&format!("modal gate {gate}, edit {edit:?}"));
            assert_eq!(f.state.borrow().pending, pending);
            assert!(!f.app.get_busy());
            f.app.set_preview_open(false);
            f.app.set_show_magnet(false);
            f.app.set_show_settings(false);
            f.app.set_confirm_open(false);
            f.app.set_ready(true);
            f.app.set_resolving_magnet(false);
            f.state.borrow_mut().pending = None;
            f.assert_allowed(edit);
        }
    }
}

#[test]
fn transfer_callbacks_reject_snapshot_discovery_and_hidden_state_before_ui_catches_up() {
    let mut f = Fixture::new();
    for edit in Edit::ALL {
        for discovery in [false, true] {
            let saved = f.state.borrow().snapshot.clone().unwrap();
            let mut snapshot = saved.as_ref().clone();
            snapshot.visible = discovery;
            snapshot.resolving_magnet = discovery;
            f.state.borrow_mut().snapshot = Some(Arc::new(snapshot));
            assert!(!f.app.get_show_magnet());
            edit.invoke(&f.app, f.id);
            f.assert_empty(if discovery {
                "metadata discovery"
            } else {
                "hidden window"
            });
            assert!(f.state.borrow().pending.is_none());
            f.state.borrow_mut().snapshot = Some(saved);
            f.assert_allowed(edit);
        }
    }
}
