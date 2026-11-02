#[path = "support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod support;
use flowtorrent::{
    app::{
        AppController, AppEvent, AppHandle,
        commands::AppCommand,
        storage::{SessionRepository, SessionStore},
    },
    domain::*,
    engine::{TorrentEngine, librqbit::LibrqbitEngine},
    error::{AppError, AppResult},
    settings::SettingsStore,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::Duration,
};
async fn command(app: &mut AppHandle, command: AppCommand) -> anyhow::Result<bool> {
    let expected = command.operation();
    app.commands.send(command).await?;
    Ok(tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Some(AppEvent::CommandFinished { operation, success }) = app.events.recv().await
                && operation == expected
            {
                return success;
            }
        }
    })
    .await?)
}
async fn ready(app: &mut AppHandle) -> anyhow::Result<()> {
    lifecycle::until(app, |s| {
        s.torrents.len() == 1 && s.torrents[0].state != TransferState::Checking
    })
    .await?;
    Ok(())
}
struct FaultStore {
    inner: SessionStore,
    mode: AtomicU8,
}
impl SessionRepository for FaultStore {
    fn load(&self) -> AppResult<Manifest> {
        self.inner.load()
    }
    fn save(&self, m: &Manifest) -> AppResult<()> {
        let mode = self.mode.load(Ordering::SeqCst);
        if mode == 1 {
            return Err(std::io::Error::other("before save").into());
        }
        self.inner.save(m)?;
        if mode == 2 {
            return Err(std::io::Error::other("after replace").into());
        }
        Ok(())
    }
}
#[tokio::test]
async fn priority_is_durable_retained_when_excluded_and_selection_never_downgrades_schema()
-> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let store = Arc::new(SessionStore::new(paths.clone()));
    let mut app = lifecycle::start(root.path(), store.clone()).await?;
    let metadata = support::multi_torrent_bytes(&[1; 32768], &[2; 65536]);
    let id = lifecycle::add(
        &mut app,
        &root.path().join("input.torrent"),
        &paths.download_dir,
        &metadata,
        None,
    )
    .await?;
    ready(&mut app).await?;
    assert!(
        command(
            &mut app,
            AppCommand::SetFilePriority(id, FileId(1), FilePriority::High)
        )
        .await?
    );
    assert_eq!(store.load()?.schema_version, 3);
    assert_eq!(
        store.load()?.torrents[0].file_priorities[0].priority,
        FilePriority::High
    );
    assert!(command(&mut app, AppCommand::SetFileEnabled(id, FileId(1), false)).await?);
    assert_eq!(store.load()?.schema_version, 3);
    assert_eq!(
        store.load()?.torrents[0].file_priorities[0].priority,
        FilePriority::High
    );
    assert!(command(&mut app, AppCommand::Pause(id)).await?);
    assert!(
        command(
            &mut app,
            AppCommand::SetFilePriority(id, FileId(1), FilePriority::Low)
        )
        .await?
    );
    assert_eq!(
        app.snapshots.borrow().torrents[0].desired_state,
        DesiredState::Paused
    );
    assert!(command(&mut app, AppCommand::InspectDetails(Some(id))).await?);
    let file = app.snapshots.borrow().details.as_ref().unwrap().files[1].clone();
    assert!(!file.selected);
    assert_eq!(file.priority, FilePriority::Low);
    lifecycle::stop(app).await?;
    let mut app = lifecycle::start(root.path(), store.clone()).await?;
    ready(&mut app).await?;
    assert!(command(&mut app, AppCommand::InspectDetails(Some(id))).await?);
    assert_eq!(
        app.snapshots.borrow().details.as_ref().unwrap().files[1].priority,
        FilePriority::Low
    );
    assert!(command(&mut app, AppCommand::SetFileEnabled(id, FileId(1), true)).await?);
    assert_eq!(store.load()?.schema_version, 3);
    assert!(
        command(
            &mut app,
            AppCommand::SetFilePriority(id, FileId(1), FilePriority::Normal)
        )
        .await?
    );
    assert!(store.load()?.torrents[0].file_priorities.is_empty());
    let before = store.load()?;
    assert!(
        !command(
            &mut app,
            AppCommand::SetFilePriority(id, FileId(9), FilePriority::High)
        )
        .await?
    );
    assert_eq!(store.load()?, before);
    lifecycle::stop(app).await
}
#[tokio::test]
async fn priority_save_errors_reconcile_old_and_replaced_policies() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let store = Arc::new(FaultStore {
        inner: SessionStore::new(paths.clone()),
        mode: AtomicU8::new(0),
    });
    let mut app = lifecycle::start(root.path(), store.clone()).await?;
    let id = lifecycle::add(
        &mut app,
        &root.path().join("input.torrent"),
        &paths.download_dir,
        &support::torrent_bytes(&support::payload(), false),
        None,
    )
    .await?;
    ready(&mut app).await?;
    store.mode.store(1, Ordering::SeqCst);
    assert!(
        !command(
            &mut app,
            AppCommand::SetFilePriority(id, FileId(0), FilePriority::High)
        )
        .await?
    );
    assert!(store.load()?.torrents[0].file_priorities.is_empty());
    assert_ne!(
        app.snapshots.borrow().torrents[0].state,
        TransferState::Paused
    );
    store.mode.store(2, Ordering::SeqCst);
    assert!(
        !command(
            &mut app,
            AppCommand::SetFilePriority(id, FileId(0), FilePriority::High)
        )
        .await?
    );
    assert_eq!(
        store.load()?.torrents[0].file_priorities[0].priority,
        FilePriority::High
    );
    assert_eq!(
        app.snapshots.borrow().torrents[0].state,
        TransferState::Error
    );
    assert!(!command(&mut app, AppCommand::Resume(id)).await?);
    store.mode.store(0, Ordering::SeqCst);
    assert!(command(&mut app, AppCommand::Retry(id)).await?);
    lifecycle::until(&mut app, |s| {
        s.torrents[0].state != TransferState::Checking && s.torrents[0].error.is_none()
    })
    .await?;
    assert!(command(&mut app, AppCommand::InspectDetails(Some(id))).await?);
    assert_eq!(
        app.snapshots.borrow().details.as_ref().unwrap().files[0].priority,
        FilePriority::High
    );
    lifecycle::stop(app).await
}
struct PriorityFault {
    inner: LibrqbitEngine,
    fail: Arc<AtomicBool>,
}
#[async_trait::async_trait]
impl TorrentEngine for PriorityFault {
    async fn inspect(&self, p: &std::path::Path) -> AppResult<TorrentPreview> {
        self.inner.inspect(p).await
    }
    async fn add(
        &mut self,
        p: &TorrentPreview,
        r: &AddRequest,
        m: AddMode,
    ) -> AppResult<TorrentId> {
        self.inner.add(p, r, m).await
    }
    async fn pause(&mut self, id: TorrentId) -> AppResult<()> {
        self.inner.pause(id).await
    }
    async fn resume(&mut self, id: TorrentId) -> AppResult<()> {
        self.inner.resume(id).await
    }
    async fn forget(&mut self, id: TorrentId) -> AppResult<()> {
        self.inner.forget(id).await
    }
    async fn snapshot(&self) -> AppResult<Vec<TorrentSnapshot>> {
        self.inner.snapshot().await
    }
    async fn details(&self, id: TorrentId) -> AppResult<Option<TorrentDetails>> {
        self.inner.details(id).await
    }
    async fn set_torrent_priority(&mut self, id: TorrentId, p: FilePriority) -> AppResult<()> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(AppError::Engine(anyhow::anyhow!("priority apply fault")));
        }
        self.inner.set_torrent_priority(id, p).await
    }
    async fn set_file_priorities(
        &mut self,
        id: TorrentId,
        p: &[FilePriorityOverride],
    ) -> AppResult<()> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(AppError::Engine(anyhow::anyhow!("priority apply fault")));
        }
        self.inner.set_file_priorities(id, p).await
    }
    async fn set_limits(&mut self, l: TransferLimits) -> AppResult<()> {
        self.inner.set_limits(l).await
    }
    async fn shutdown(&mut self) -> AppResult<()> {
        self.inner.shutdown().await
    }
}
#[tokio::test]
async fn priority_engine_failure_blocks_stale_resume_and_retry_configures_before_unpause()
-> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.download_dir)?;
    let store = Arc::new(SessionStore::new(paths.clone()));
    let fail = Arc::new(AtomicBool::new(true));
    let engine = PriorityFault {
        inner: LibrqbitEngine::with_options(
            paths.download_dir.clone(),
            support::local_options(),
            vec![],
        )
        .await?,
        fail: fail.clone(),
    };
    let mut app = AppController::spawn(
        Box::new(engine),
        store.clone(),
        Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        )),
        paths.clone(),
    );
    let id = lifecycle::add(
        &mut app,
        &root.path().join("input.torrent"),
        &paths.download_dir,
        &support::torrent_bytes(&support::payload(), false),
        None,
    )
    .await?;
    ready(&mut app).await?;
    assert!(
        !command(
            &mut app,
            AppCommand::SetFilePriority(id, FileId(0), FilePriority::High)
        )
        .await?
    );
    assert_eq!(
        app.snapshots.borrow().torrents[0].state,
        TransferState::Error
    );
    assert!(!command(&mut app, AppCommand::Resume(id)).await?);
    fail.store(false, Ordering::SeqCst);
    assert!(command(&mut app, AppCommand::Retry(id)).await?);
    lifecycle::until(&mut app, |s| {
        s.torrents[0].state != TransferState::Checking && s.torrents[0].error.is_none()
    })
    .await?;
    assert!(command(&mut app, AppCommand::InspectDetails(Some(id))).await?);
    assert_eq!(
        app.snapshots.borrow().details.as_ref().unwrap().files[0].priority,
        FilePriority::High
    );
    lifecycle::stop(app).await
}

#[tokio::test]
async fn controller_rejects_initial_priority_fields_instead_of_applying_unpersisted_policy()
-> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let store = Arc::new(SessionStore::new(paths.clone()));
    let mut app = lifecycle::start(root.path(), store.clone()).await?;
    let source = root.path().join("input.torrent");
    std::fs::write(&source, support::torrent_bytes(&support::payload(), false))?;
    assert!(command(&mut app, AppCommand::OpenTorrent(source)).await?);
    let preview = app.snapshots.borrow().preview.as_ref().unwrap().clone();
    let request = AddRequest::new(
        preview.id,
        paths.download_dir.clone(),
        preview.default_selection(),
    )
    .with_file_priorities(vec![FilePriorityOverride {
        file: FileId(0),
        priority: FilePriority::High,
    }]);
    assert!(!command(&mut app, AppCommand::ConfirmAdd(request)).await?);
    assert!(store.load()?.torrents.is_empty());
    assert!(app.snapshots.borrow().preview.is_some());
    assert!(std::fs::read_dir(&paths.download_dir)?.next().is_none());
    lifecycle::stop(app).await
}

struct ThirdIntentStore {
    inner: SessionStore,
    armed: AtomicBool,
    complete: Arc<AtomicBool>,
    saves: std::sync::atomic::AtomicUsize,
}
impl SessionRepository for ThirdIntentStore {
    fn load(&self) -> AppResult<Manifest> {
        self.inner.load()
    }
    fn save(&self, m: &Manifest) -> AppResult<()> {
        self.saves.fetch_add(1, Ordering::SeqCst);
        if self.armed.swap(false, Ordering::SeqCst) {
            let mut third = m.clone();
            third.torrents[0].file_priorities = vec![FilePriorityOverride {
                file: FileId(0),
                priority: FilePriority::Low,
            }];
            third.torrents[1].desired_state = DesiredState::Paused;
            self.inner.save(&third)?;
            self.complete.store(true, Ordering::SeqCst);
            return Err(std::io::Error::other("unresolved third intent after replacement").into());
        }
        self.inner.save(m)
    }
}
struct CompletingEngine {
    inner: LibrqbitEngine,
    complete: Arc<AtomicBool>,
    hold_checking: bool,
    second_resumes: Arc<std::sync::atomic::AtomicUsize>,
}
#[async_trait::async_trait]
impl TorrentEngine for CompletingEngine {
    async fn inspect(&self, p: &std::path::Path) -> AppResult<TorrentPreview> {
        self.inner.inspect(p).await
    }
    async fn add(
        &mut self,
        p: &TorrentPreview,
        r: &AddRequest,
        m: AddMode,
    ) -> AppResult<TorrentId> {
        self.inner.add(p, r, m).await
    }
    async fn pause(&mut self, id: TorrentId) -> AppResult<()> {
        self.inner.pause(id).await
    }
    async fn resume(&mut self, id: TorrentId) -> AppResult<()> {
        if self
            .inner
            .snapshot()
            .await?
            .iter()
            .any(|row| row.id == id && row.destination.ends_with("second"))
        {
            self.second_resumes.fetch_add(1, Ordering::SeqCst);
        }
        self.inner.resume(id).await
    }
    async fn forget(&mut self, id: TorrentId) -> AppResult<()> {
        self.inner.forget(id).await
    }
    async fn snapshot(&self) -> AppResult<Vec<TorrentSnapshot>> {
        let mut rows = self.inner.snapshot().await?;
        for row in &mut rows {
            if row.destination.ends_with("second") {
                if self.hold_checking {
                    row.state = if self.complete.load(Ordering::SeqCst) {
                        TransferState::Paused
                    } else {
                        TransferState::Checking
                    };
                } else if self.complete.load(Ordering::SeqCst) {
                    row.state = TransferState::Seeding;
                    row.checked_downloaded_bytes = row.selected_bytes;
                }
            }
        }
        Ok(rows)
    }
    async fn details(&self, id: TorrentId) -> AppResult<Option<TorrentDetails>> {
        self.inner.details(id).await
    }
    async fn set_file_priorities(
        &mut self,
        id: TorrentId,
        p: &[FilePriorityOverride],
    ) -> AppResult<()> {
        self.inner.set_file_priorities(id, p).await
    }
    async fn set_limits(&mut self, l: TransferLimits) -> AppResult<()> {
        self.inner.set_limits(l).await
    }
    async fn shutdown(&mut self) -> AppResult<()> {
        self.inner.shutdown().await
    }
}
#[tokio::test]
async fn unresolved_read_only_intent_cannot_be_overwritten_by_another_completion()
-> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.download_dir)?;
    let complete = Arc::new(AtomicBool::new(false));
    let store = Arc::new(ThirdIntentStore {
        inner: SessionStore::new(paths.clone()),
        armed: AtomicBool::new(false),
        complete: complete.clone(),
        saves: std::sync::atomic::AtomicUsize::new(0),
    });
    let engine = CompletingEngine {
        inner: LibrqbitEngine::with_options(
            paths.download_dir.clone(),
            support::local_options(),
            vec![],
        )
        .await?,
        complete,
        hold_checking: false,
        second_resumes: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    };
    let mut app = AppController::spawn(
        Box::new(engine),
        store.clone(),
        Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        )),
        paths.clone(),
    );
    let id = lifecycle::add(
        &mut app,
        &root.path().join("first.torrent"),
        &paths.download_dir,
        &support::torrent_bytes(&support::payload(), false),
        None,
    )
    .await?;
    ready(&mut app).await?;
    let second = paths.download_dir.join("second");
    std::fs::create_dir(&second)?;
    lifecycle::add(
        &mut app,
        &root.path().join("second.torrent"),
        &second,
        &support::torrent_bytes(&support::payload(), true),
        None,
    )
    .await?;
    lifecycle::until(&mut app, |s| {
        s.torrents
            .iter()
            .all(|r| r.state != TransferState::Checking)
    })
    .await?;
    let saves = store.saves.load(Ordering::SeqCst);
    store.armed.store(true, Ordering::SeqCst);
    assert!(
        !command(
            &mut app,
            AppCommand::SetFilePriority(id, FileId(0), FilePriority::High)
        )
        .await?
    );
    assert!(app.snapshots.borrow().blocked_state_path.is_some());
    let disk = store.load()?;
    assert_eq!(
        disk.torrents[0].file_priorities[0].priority,
        FilePriority::Low,
        "completion overwrote unresolved intent"
    );
    assert_eq!(disk.torrents[1].desired_state, DesiredState::Paused);
    assert!(!disk.torrents[1].completion_notified);
    tokio::time::sleep(Duration::from_millis(650)).await;
    assert!(command(&mut app, AppCommand::SetVisible(false)).await?);
    assert_eq!(
        store.saves.load(Ordering::SeqCst),
        saves + 1,
        "read-only timer persisted completion"
    );
    assert_eq!(store.load()?, disk);
    lifecycle::stop(app).await
}

#[tokio::test]
async fn unresolved_read_only_intent_cannot_auto_resume_a_checking_torrent() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.download_dir)?;
    let complete = Arc::new(AtomicBool::new(false));
    let resumes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let store = Arc::new(ThirdIntentStore {
        inner: SessionStore::new(paths.clone()),
        armed: AtomicBool::new(false),
        complete: complete.clone(),
        saves: std::sync::atomic::AtomicUsize::new(0),
    });
    let engine = CompletingEngine {
        inner: LibrqbitEngine::with_options(
            paths.download_dir.clone(),
            support::local_options(),
            vec![],
        )
        .await?,
        complete,
        hold_checking: true,
        second_resumes: resumes.clone(),
    };
    let mut app = AppController::spawn(
        Box::new(engine),
        store.clone(),
        Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        )),
        paths.clone(),
    );
    let id = lifecycle::add(
        &mut app,
        &root.path().join("first.torrent"),
        &paths.download_dir,
        &support::torrent_bytes(&support::payload(), false),
        None,
    )
    .await?;
    ready(&mut app).await?;
    let second = paths.download_dir.join("second");
    std::fs::create_dir(&second)?;
    lifecycle::add(
        &mut app,
        &root.path().join("second.torrent"),
        &second,
        &support::torrent_bytes(&support::payload(), true),
        None,
    )
    .await?;
    assert_eq!(resumes.load(Ordering::SeqCst), 0);
    store.armed.store(true, Ordering::SeqCst);
    assert!(
        !command(
            &mut app,
            AppCommand::SetFilePriority(id, FileId(0), FilePriority::High)
        )
        .await?
    );
    assert!(app.snapshots.borrow().blocked_state_path.is_some());
    assert_eq!(
        resumes.load(Ordering::SeqCst),
        0,
        "read-only checking resumed stale intent"
    );
    lifecycle::stop(app).await
}

#[tokio::test]
async fn torrent_priority_is_independent_durable_and_survives_file_edits_and_restart()
-> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let store = Arc::new(SessionStore::new(paths.clone()));
    let mut app = lifecycle::start(root.path(), store.clone()).await?;
    let id = lifecycle::add(
        &mut app,
        &root.path().join("input.torrent"),
        &paths.download_dir,
        &support::multi_torrent_bytes(&[1; 32768], &[2; 65536]),
        None,
    )
    .await?;
    ready(&mut app).await?;
    assert!(
        command(
            &mut app,
            AppCommand::SetTorrentPriority(id, FilePriority::High)
        )
        .await?
    );
    assert_eq!(store.load()?.schema_version, 4);
    assert_eq!(store.load()?.torrents[0].priority, FilePriority::High);
    assert_eq!(
        app.snapshots.borrow().torrents[0].priority,
        FilePriority::High
    );
    assert!(
        command(
            &mut app,
            AppCommand::SetFilePriority(id, FileId(1), FilePriority::Low)
        )
        .await?
    );
    assert!(command(&mut app, AppCommand::SetFileEnabled(id, FileId(1), false)).await?);
    assert_eq!(store.load()?.schema_version, 4);
    assert_eq!(store.load()?.torrents[0].priority, FilePriority::High);
    assert!(command(&mut app, AppCommand::Pause(id)).await?);
    lifecycle::stop(app).await?;
    let mut app = lifecycle::start(root.path(), store.clone()).await?;
    ready(&mut app).await?;
    assert_eq!(
        app.snapshots.borrow().torrents[0].priority,
        FilePriority::High
    );
    assert_eq!(
        app.snapshots.borrow().torrents[0].desired_state,
        DesiredState::Paused
    );
    assert!(
        command(
            &mut app,
            AppCommand::SetTorrentPriority(id, FilePriority::Normal)
        )
        .await?
    );
    assert_eq!(store.load()?.schema_version, 4);
    assert_eq!(
        store.load()?.torrents[0].file_priorities[0].priority,
        FilePriority::Low
    );
    let before = store.load()?;
    assert!(
        !command(
            &mut app,
            AppCommand::SetTorrentPriority(TorrentId([99; 20]), FilePriority::High)
        )
        .await?
    );
    assert_eq!(store.load()?, before);
    lifecycle::stop(app).await
}

#[tokio::test]
async fn torrent_priority_save_errors_reconcile_old_and_replaced_policies() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let store = Arc::new(FaultStore {
        inner: SessionStore::new(paths.clone()),
        mode: AtomicU8::new(0),
    });
    let mut app = lifecycle::start(root.path(), store.clone()).await?;
    let id = lifecycle::add(
        &mut app,
        &root.path().join("input.torrent"),
        &paths.download_dir,
        &support::torrent_bytes(&support::payload(), false),
        None,
    )
    .await?;
    ready(&mut app).await?;
    store.mode.store(1, Ordering::SeqCst);
    assert!(
        !command(
            &mut app,
            AppCommand::SetTorrentPriority(id, FilePriority::High)
        )
        .await?
    );
    assert_eq!(store.load()?.torrents[0].priority, FilePriority::Normal);
    assert_ne!(
        app.snapshots.borrow().torrents[0].state,
        TransferState::Paused
    );
    store.mode.store(2, Ordering::SeqCst);
    assert!(
        !command(
            &mut app,
            AppCommand::SetTorrentPriority(id, FilePriority::High)
        )
        .await?
    );
    assert_eq!(store.load()?.torrents[0].priority, FilePriority::High);
    assert_eq!(
        app.snapshots.borrow().torrents[0].state,
        TransferState::Error
    );
    assert!(!command(&mut app, AppCommand::Resume(id)).await?);
    store.mode.store(0, Ordering::SeqCst);
    assert!(command(&mut app, AppCommand::Retry(id)).await?);
    lifecycle::until(&mut app, |s| {
        s.torrents[0].state != TransferState::Checking && s.torrents[0].error.is_none()
    })
    .await?;
    assert!(command(&mut app, AppCommand::InspectDetails(Some(id))).await?);
    assert_eq!(
        app.snapshots.borrow().torrents[0].priority,
        FilePriority::High
    );
    lifecycle::stop(app).await
}

#[tokio::test]
async fn torrent_priority_engine_failure_blocks_stale_resume_and_retry_configures_before_unpause()
-> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.download_dir)?;
    let store = Arc::new(SessionStore::new(paths.clone()));
    let fail = Arc::new(AtomicBool::new(true));
    let engine = PriorityFault {
        inner: LibrqbitEngine::with_options(
            paths.download_dir.clone(),
            support::local_options(),
            vec![],
        )
        .await?,
        fail: fail.clone(),
    };
    let mut app = AppController::spawn(
        Box::new(engine),
        store.clone(),
        Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        )),
        paths.clone(),
    );
    let id = lifecycle::add(
        &mut app,
        &root.path().join("input.torrent"),
        &paths.download_dir,
        &support::torrent_bytes(&support::payload(), false),
        None,
    )
    .await?;
    ready(&mut app).await?;
    assert!(
        !command(
            &mut app,
            AppCommand::SetTorrentPriority(id, FilePriority::High)
        )
        .await?
    );
    assert_eq!(
        app.snapshots.borrow().torrents[0].state,
        TransferState::Error
    );
    assert!(!command(&mut app, AppCommand::Resume(id)).await?);
    fail.store(false, Ordering::SeqCst);
    assert!(command(&mut app, AppCommand::Retry(id)).await?);
    lifecycle::until(&mut app, |s| {
        s.torrents[0].state != TransferState::Checking && s.torrents[0].error.is_none()
    })
    .await?;
    assert!(command(&mut app, AppCommand::InspectDetails(Some(id))).await?);
    assert_eq!(
        app.snapshots.borrow().torrents[0].priority,
        FilePriority::High
    );
    lifecycle::stop(app).await
}
