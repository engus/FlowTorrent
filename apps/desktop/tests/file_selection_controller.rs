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
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

struct FailingStore {
    inner: SessionStore,
    fail: AtomicBool,
}
impl SessionRepository for FailingStore {
    fn load(&self) -> AppResult<Manifest> {
        self.inner.load()
    }
    fn save(&self, manifest: &Manifest) -> AppResult<()> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("selection save fixture").into());
        }
        self.inner.save(manifest)
    }
}
async fn command(app: &mut AppHandle, command: AppCommand) -> anyhow::Result<bool> {
    let expected = command.operation();
    app.commands.send(command).await?;
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Some(AppEvent::CommandFinished { operation, success }) = app.events.recv().await
                && operation == expected
            {
                return success;
            }
        }
    })
    .await
    .map_err(Into::into)
}
async fn ready(app: &mut AppHandle) -> anyhow::Result<()> {
    lifecycle::until(app, |s| {
        s.torrents.len() == 1 && s.torrents[0].state != TransferState::Checking
    })
    .await?;
    Ok(())
}
#[tokio::test]
async fn serialized_toggles_empty_restart_and_failed_save_preserve_policy() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let store = Arc::new(FailingStore {
        inner: SessionStore::new(paths.clone()),
        fail: AtomicBool::new(false),
    });
    let mut app = lifecycle::start(root.path(), store.clone()).await?;
    let metadata = support::multi_torrent_bytes(&[1; 32_768], &[2; 65_536]);
    let id = lifecycle::add(
        &mut app,
        &root.path().join("input.torrent"),
        &paths.download_dir,
        &metadata,
        None,
    )
    .await?;
    ready(&mut app).await?;
    assert!(command(&mut app, AppCommand::Pause(id)).await?);
    assert!(command(&mut app, AppCommand::SetFileEnabled(id, FileId(0), false)).await?);
    assert_eq!(store.load()?.torrents[0].selected_files, vec![FileId(1)]);
    assert_eq!(
        app.snapshots.borrow().torrents[0].desired_state,
        DesiredState::Paused
    );
    assert_eq!(
        app.snapshots.borrow().torrents[0].state,
        TransferState::Paused
    );
    store.fail.store(true, Ordering::SeqCst);
    assert!(!command(&mut app, AppCommand::SetFileEnabled(id, FileId(1), false)).await?);
    assert_eq!(store.load()?.torrents[0].selected_files, vec![FileId(1)]);
    store.fail.store(false, Ordering::SeqCst);
    assert!(command(&mut app, AppCommand::SetFileEnabled(id, FileId(1), false)).await?);
    assert_eq!(
        app.snapshots.borrow().torrents[0].state,
        TransferState::NoFilesSelected
    );
    assert!(!store.load()?.torrents[0].completion_notified);
    assert!(command(&mut app, AppCommand::Resume(id)).await?);
    lifecycle::stop(app).await?;
    let mut app = lifecycle::start(root.path(), store.clone()).await?;
    ready(&mut app).await?;
    assert_eq!(
        app.snapshots.borrow().torrents[0].state,
        TransferState::NoFilesSelected
    );
    assert_eq!(
        app.snapshots.borrow().torrents[0].desired_state,
        DesiredState::Active
    );
    // Both commands are queued before either response: no stale whole-list overwrite.
    app.commands
        .send(AppCommand::SetFileEnabled(id, FileId(0), true))
        .await?;
    app.commands
        .send(AppCommand::SetFileEnabled(id, FileId(1), true))
        .await?;
    lifecycle::until(&mut app, |s| {
        s.torrents[0].selected_bytes == 98_304 && s.torrents[0].error.is_none()
    })
    .await?;
    assert_eq!(
        store.load()?.torrents[0].selected_files,
        vec![FileId(0), FileId(1)]
    );
    assert_eq!(
        store.load()?.torrents[0].desired_state,
        DesiredState::Active
    );
    assert!(command(&mut app, AppCommand::InspectDetails(None)).await?);
    let before = store.load()?;
    assert!(!command(&mut app, AppCommand::SetFileEnabled(id, FileId(9), true)).await?);
    assert_eq!(store.load()?, before);
    // Failed active save restores the old runtime policy and keeps intent active.
    store.fail.store(true, Ordering::SeqCst);
    assert!(!command(&mut app, AppCommand::SetFileEnabled(id, FileId(0), false)).await?);
    assert_eq!(store.load()?, before);
    assert_ne!(
        app.snapshots.borrow().torrents[0].state,
        TransferState::Paused
    );
    store.fail.store(false, Ordering::SeqCst);
    lifecycle::stop(app).await?;
    assert!(paths.download_dir.join("bundle/first.bin").is_file());
    assert!(paths.download_dir.join("bundle/second.bin").is_file());
    Ok(())
}

struct SelectionFault {
    inner: LibrqbitEngine,
    fail: Arc<AtomicBool>,
}
#[async_trait::async_trait]
impl TorrentEngine for SelectionFault {
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
    async fn set_file_selection(&mut self, id: TorrentId, files: &[FileId]) -> AppResult<()> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(AppError::Engine(anyhow::anyhow!("selection fault")));
        }
        self.inner.set_file_selection(id, files).await
    }
    async fn set_limits(&mut self, l: TransferLimits) -> AppResult<()> {
        self.inner.set_limits(l).await
    }
    async fn shutdown(&mut self) -> AppResult<()> {
        self.inner.shutdown().await
    }
}
#[tokio::test]
async fn engine_failure_after_save_blocks_stale_resume_and_retry_restores_durable_selection()
-> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.download_dir)?;
    let store = Arc::new(SessionStore::new(paths.clone()));
    let fail = Arc::new(AtomicBool::new(true));
    let engine = SelectionFault {
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
        &support::multi_torrent_bytes(&[1; 32_768], &[2; 65_536]),
        None,
    )
    .await?;
    ready(&mut app).await?;
    assert!(!command(&mut app, AppCommand::SetFileEnabled(id, FileId(0), false)).await?);
    assert_eq!(store.load()?.torrents[0].selected_files, vec![FileId(1)]);
    assert_eq!(
        app.snapshots.borrow().torrents[0].state,
        TransferState::Error
    );
    assert!(!command(&mut app, AppCommand::Resume(id)).await?);
    assert!(command(&mut app, AppCommand::Pause(id)).await?);
    assert_eq!(
        app.snapshots.borrow().torrents[0].state,
        TransferState::Error
    );
    fail.store(false, Ordering::SeqCst);
    assert!(command(&mut app, AppCommand::Retry(id)).await?);
    lifecycle::until(&mut app, |s| {
        s.torrents[0].state != TransferState::Checking && s.torrents[0].error.is_none()
    })
    .await?;
    assert_eq!(app.snapshots.borrow().torrents[0].selected_bytes, 65_536);
    assert_eq!(
        store.load()?.torrents[0].desired_state,
        DesiredState::Paused
    );
    assert_eq!(
        app.snapshots.borrow().torrents[0].desired_state,
        DesiredState::Paused
    );
    assert!(command(&mut app, AppCommand::Resume(id)).await?);
    assert!(command(&mut app, AppCommand::InspectDetails(Some(id))).await?);
    assert_eq!(
        app.snapshots
            .borrow()
            .details
            .as_ref()
            .unwrap()
            .files
            .iter()
            .map(|f| f.selected)
            .collect::<Vec<_>>(),
        vec![false, true]
    );
    lifecycle::stop(app).await
}

#[tokio::test]
async fn trash_moves_previously_selected_owned_files_even_with_empty_selection()
-> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.download_dir)?;
    let bin = root.path().join("fake-trash");
    let store = Arc::new(SessionStore::new(paths.clone()));
    let engine =
        LibrqbitEngine::with_options(paths.download_dir.clone(), support::local_options(), vec![])
            .await?;
    let destination = bin.clone();
    let mut app = AppController::spawn_with_trash(
        Box::new(engine),
        store.clone(),
        Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        )),
        paths.clone(),
        Arc::new(move |path| {
            std::fs::create_dir_all(&destination)?;
            std::fs::rename(path, destination.join(path.file_name().unwrap()))?;
            Ok(())
        }),
    );
    let id = lifecycle::add(
        &mut app,
        &root.path().join("input.torrent"),
        &paths.download_dir,
        &support::multi_torrent_bytes(&[1; 32_768], &[2; 65_536]),
        None,
    )
    .await?;
    ready(&mut app).await?;
    assert!(command(&mut app, AppCommand::Pause(id)).await?);
    assert!(command(&mut app, AppCommand::SetFileEnabled(id, FileId(0), false)).await?);
    assert!(command(&mut app, AppCommand::SetFileEnabled(id, FileId(1), false)).await?);
    std::fs::write(paths.download_dir.join("bundle/mine.txt"), b"foreign")?;
    assert!(command(&mut app, AppCommand::Trash(id)).await?);
    assert!(store.load()?.torrents.is_empty());
    assert_eq!(
        std::fs::read(paths.download_dir.join("bundle/mine.txt"))?,
        b"foreign"
    );
    assert!(!paths.download_dir.join("bundle/first.bin").exists());
    let files = std::fs::read_dir(&bin)?.next().unwrap()?.path();
    assert!(files.join("bundle/first.bin").is_file() || files.join("first.bin").is_file());
    lifecycle::stop(app).await
}

#[tokio::test]
async fn empty_paused_torrent_can_be_removed_without_deleting_payload() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let store = Arc::new(SessionStore::new(paths.clone()));
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
    assert!(command(&mut app, AppCommand::Pause(id)).await?);
    assert!(command(&mut app, AppCommand::SetFileEnabled(id, FileId(0), false)).await?);
    assert!(command(&mut app, AppCommand::Forget(id)).await?);
    assert!(store.load()?.torrents.is_empty());
    assert!(paths.download_dir.join("fixture.bin").exists());
    lifecycle::stop(app).await
}

#[tokio::test]
async fn removal_clears_selection_recovery_before_readding_same_infohash() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.download_dir)?;
    let store = Arc::new(SessionStore::new(paths.clone()));
    let fail = Arc::new(AtomicBool::new(true));
    let engine = SelectionFault {
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
    let metadata = support::multi_torrent_bytes(&[1; 32_768], &[2; 65_536]);
    let id = lifecycle::add(
        &mut app,
        &root.path().join("input.torrent"),
        &paths.download_dir,
        &metadata,
        None,
    )
    .await?;
    ready(&mut app).await?;
    assert!(!command(&mut app, AppCommand::SetFileEnabled(id, FileId(0), false)).await?);
    assert!(command(&mut app, AppCommand::Forget(id)).await?);
    fail.store(false, Ordering::SeqCst);
    let other = root.path().join("other");
    std::fs::create_dir(&other)?;
    lifecycle::add(
        &mut app,
        &root.path().join("input.torrent"),
        &other,
        &metadata,
        None,
    )
    .await?;
    ready(&mut app).await?;
    assert!(command(&mut app, AppCommand::Pause(id)).await?);
    assert!(command(&mut app, AppCommand::Resume(id)).await?);
    lifecycle::stop(app).await
}

struct AfterReplaceStore {
    inner: SessionStore,
    fail_after: AtomicBool,
}
impl SessionRepository for AfterReplaceStore {
    fn load(&self) -> AppResult<Manifest> {
        self.inner.load()
    }
    fn save(&self, manifest: &Manifest) -> AppResult<()> {
        self.inner.save(manifest)?;
        if self.fail_after.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("parent sync failed after replace").into());
        }
        Ok(())
    }
}
#[tokio::test]
async fn ambiguous_save_keeps_runtime_paused_and_recovers_the_observed_durable_selection()
-> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let store = Arc::new(AfterReplaceStore {
        inner: SessionStore::new(paths.clone()),
        fail_after: AtomicBool::new(false),
    });
    let mut app = lifecycle::start(root.path(), store.clone()).await?;
    let id = lifecycle::add(
        &mut app,
        &root.path().join("input.torrent"),
        &paths.download_dir,
        &support::multi_torrent_bytes(&[1; 32_768], &[2; 65_536]),
        None,
    )
    .await?;
    ready(&mut app).await?;
    store.fail_after.store(true, Ordering::SeqCst);
    assert!(!command(&mut app, AppCommand::SetFileEnabled(id, FileId(0), false)).await?);
    assert_eq!(store.load()?.torrents[0].selected_files, vec![FileId(1)]);
    assert_eq!(
        app.snapshots.borrow().torrents[0].state,
        TransferState::Error
    );
    assert!(!command(&mut app, AppCommand::Resume(id)).await?);
    store.fail_after.store(false, Ordering::SeqCst);
    assert!(command(&mut app, AppCommand::Retry(id)).await?);
    lifecycle::until(&mut app, |s| {
        s.torrents[0].state != TransferState::Checking && s.torrents[0].error.is_none()
    })
    .await?;
    assert_eq!(app.snapshots.borrow().torrents[0].selected_bytes, 65_536);
    lifecycle::stop(app).await
}
