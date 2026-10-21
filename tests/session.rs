#[allow(dead_code)]
mod support;

use flowtorrent::{
    app::{
        AppController, AppHandle, AppSnapshot, commands::AppCommand, storage::SessionRepository,
    },
    domain::*,
    engine::librqbit::LibrqbitEngine,
    error::{AppError, AppResult},
    paths::AppPaths,
    settings::SettingsStore,
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Default)]
struct MemoryStore {
    manifest: Mutex<Manifest>,
    fail: AtomicBool,
}
impl SessionRepository for MemoryStore {
    fn load(&self) -> AppResult<Manifest> {
        Ok(self.manifest.lock().unwrap().clone())
    }
    fn save(&self, manifest: &Manifest) -> AppResult<()> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(AppError::Io(std::io::Error::other("test disk unavailable")));
        }
        *self.manifest.lock().unwrap() = manifest.clone();
        Ok(())
    }
}

fn paths(root: &std::path::Path) -> AppPaths {
    AppPaths {
        config_file: root.join("state/settings.json"),
        data_dir: root.join("state"),
        metadata_dir: root.join("state/metadata"),
        manifest_file: root.join("state/session.json"),
        download_dir: root.join("downloads"),
    }
}
async fn start(root: &std::path::Path, store: Arc<MemoryStore>) -> anyhow::Result<AppHandle> {
    let paths = paths(root);
    std::fs::create_dir_all(&paths.download_dir)?;
    let engine =
        LibrqbitEngine::with_options(paths.download_dir.clone(), support::local_options(), vec![])
            .await?;
    Ok(AppController::spawn(
        Box::new(engine),
        store,
        Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        )),
        paths,
    ))
}
async fn until(
    handle: &mut AppHandle,
    predicate: impl Fn(&AppSnapshot) -> bool,
) -> anyhow::Result<AppSnapshot> {
    tokio::time::timeout(Duration::from_secs(15),async {
        loop {
            let snapshot=handle.snapshots.borrow().clone();
            if predicate(&snapshot) { return Ok(snapshot); }
            tokio::select! { result=handle.snapshots.changed()=>result?, _=handle.events.recv()=>{} }
        }
    }).await?
}
async fn preview(handle: &mut AppHandle, path: PathBuf) -> anyhow::Result<TorrentPreview> {
    handle.commands.send(AppCommand::OpenTorrent(path)).await?;
    Ok(until(handle, |s| s.preview.is_some())
        .await?
        .preview
        .unwrap())
}
async fn stop(mut handle: AppHandle) -> anyhow::Result<()> {
    handle.commands.send(AppCommand::Shutdown).await?;
    tokio::time::timeout(Duration::from_secs(15), async {
        while let Some(event) = handle.events.recv().await {
            if matches!(event, flowtorrent::app::AppEvent::ShutdownReady) {
                return;
            }
        }
    })
    .await?;
    Ok(())
}

#[tokio::test]
async fn confirmation_creates_one_durable_row_and_pause_survives_snapshots() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let store = Arc::new(MemoryStore::default());
    let mut handle = start(root.path(), store.clone()).await?;
    let path = root.path().join("input.torrent");
    std::fs::write(&path, support::torrent_bytes(&support::payload(), false))?;
    let item = preview(&mut handle, path.clone()).await?;
    assert!(
        std::fs::read_dir(root.path().join("downloads"))?
            .next()
            .is_none()
    );
    assert!(store.load()?.torrents.is_empty());
    handle
        .commands
        .send(AppCommand::ConfirmAdd(AddRequest::new(
            item.id,
            root.path().join("downloads"),
            item.default_selection(),
        )))
        .await?;
    until(&mut handle, |s| {
        s.torrents.len() == 1 && s.preview.is_none()
    })
    .await?;
    let record = store.load()?.torrents.remove(0);
    assert_eq!(record.id, item.id);
    assert_eq!(std::fs::read(record.metadata_path)?, std::fs::read(path)?);
    handle.commands.send(AppCommand::Pause(item.id)).await?;
    let paused = until(&mut handle, |s| {
        s.torrents[0].state == TransferState::Paused
    })
    .await?;
    assert_eq!(paused.torrents[0].download_bps, 0);
    assert_eq!(paused.torrents[0].upload_bps, 0);
    assert_eq!(
        store.load()?.torrents[0].desired_state,
        DesiredState::Paused
    );
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert_eq!(
        handle.snapshots.borrow().torrents[0].state,
        TransferState::Paused
    );
    handle.commands.send(AppCommand::Resume(item.id)).await?;
    until(&mut handle, |s| {
        matches!(
            s.torrents[0].state,
            TransferState::WaitingForSources | TransferState::Connecting
        )
    })
    .await?;
    assert_eq!(
        store.load()?.torrents[0].desired_state,
        DesiredState::Active
    );
    stop(handle).await
}

#[tokio::test]
async fn failed_add_keeps_an_explainable_error_without_clobbering_payload() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let store = Arc::new(MemoryStore::default());
    let mut handle = start(root.path(), store.clone()).await?;
    let path = root.path().join("input.torrent");
    std::fs::write(&path, support::torrent_bytes(&support::payload(), false))?;
    let item = preview(&mut handle, path).await?;
    let existing = root.path().join("downloads/fixture.bin");
    std::fs::write(&existing, b"mine")?;
    handle
        .commands
        .send(AppCommand::ConfirmAdd(AddRequest::new(
            item.id,
            root.path().join("downloads"),
            item.default_selection(),
        )))
        .await?;
    let state = until(&mut handle, |s| s.error.is_some()).await?;
    assert_eq!(state.error.unwrap().code, ErrorCode::FileConflict);
    assert!(state.preview.is_some());
    assert!(store.load()?.torrents.is_empty());
    assert_eq!(std::fs::read(existing)?, b"mine");
    stop(handle).await
}

#[tokio::test]
async fn persistence_failure_never_acknowledges_new_download() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let store = Arc::new(MemoryStore::default());
    let mut handle = start(root.path(), store.clone()).await?;
    let path = root.path().join("input.torrent");
    std::fs::write(&path, support::torrent_bytes(&support::payload(), false))?;
    let item = preview(&mut handle, path).await?;
    store.fail.store(true, Ordering::SeqCst);
    handle
        .commands
        .send(AppCommand::ConfirmAdd(AddRequest::new(
            item.id,
            root.path().join("downloads"),
            item.default_selection(),
        )))
        .await?;
    let state = until(&mut handle, |s| s.error.is_some()).await?;
    assert!(state.torrents.is_empty());
    assert!(state.preview.is_some());
    assert!(store.load()?.torrents.is_empty());
    assert_eq!(
        std::fs::metadata(root.path().join("downloads/fixture.bin"))?.len(),
        0
    );
    store.fail.store(false, Ordering::SeqCst);
    // Retry this confirmation must retain its own claimed handles.
    handle
        .commands
        .send(AppCommand::ConfirmAdd(AddRequest::new(
            item.id,
            root.path().join("downloads"),
            item.default_selection(),
        )))
        .await?;
    until(&mut handle, |s| {
        s.torrents.len() == 1 && s.preview.is_none()
    })
    .await?;
    stop(handle).await
}

#[tokio::test]
async fn remove_keeps_payload_and_durably_removes_the_row() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let store = Arc::new(MemoryStore::default());
    let mut handle = start(root.path(), store.clone()).await?;
    let path = root.path().join("input.torrent");
    std::fs::write(&path, support::torrent_bytes(&support::payload(), false))?;
    let item = preview(&mut handle, path).await?;
    handle
        .commands
        .send(AppCommand::ConfirmAdd(AddRequest::new(
            item.id,
            root.path().join("downloads"),
            item.default_selection(),
        )))
        .await?;
    until(&mut handle, |s| {
        s.torrents.len() == 1 && s.preview.is_none()
    })
    .await?;
    handle.commands.send(AppCommand::Forget(item.id)).await?;
    until(&mut handle, |s| s.torrents.is_empty()).await?;
    assert!(store.load()?.torrents.is_empty());
    assert!(root.path().join("downloads/fixture.bin").exists());
    stop(handle).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn completed_download_keeps_seeding_and_pause_becomes_completed() -> anyhow::Result<()> {
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    let payload = support::payload();
    let metadata = support::torrent_bytes(&payload, false);
    let (seeder, peer) = support::seed(source.path(), &metadata, &payload).await?;
    let paths = paths(target.path());
    std::fs::create_dir(&paths.download_dir)?;
    let engine = LibrqbitEngine::with_options(
        paths.download_dir.clone(),
        support::local_options(),
        vec![peer],
    )
    .await?;
    let mut handle = AppController::spawn(
        Box::new(engine),
        Arc::new(MemoryStore::default()),
        Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        )),
        paths,
    );
    let path = source.path().join("input.torrent");
    std::fs::write(&path, metadata)?;
    let item = preview(&mut handle, path).await?;
    handle
        .commands
        .send(AppCommand::ConfirmAdd(AddRequest::new(
            item.id,
            target.path().join("downloads"),
            item.default_selection(),
        )))
        .await?;
    let complete = until(&mut handle, |s| {
        s.torrents
            .first()
            .is_some_and(|r| r.state == TransferState::Seeding)
    })
    .await?;
    assert_eq!(complete.torrents[0].selected_bytes, 524_288);
    assert_eq!(complete.torrents[0].checked_downloaded_bytes, 524_288);
    assert_eq!(
        std::fs::read(target.path().join("downloads/fixture.bin"))?,
        payload
    );
    handle.commands.send(AppCommand::Pause(item.id)).await?;
    let paused = until(&mut handle, |s| {
        s.torrents[0].state == TransferState::Completed
    })
    .await?;
    assert_eq!(paused.torrents[0].state, TransferState::Completed);
    stop(handle).await?;
    seeder.stop().await;
    Ok(())
}

struct FailFirstAdd {
    inner: LibrqbitEngine,
    fail: bool,
}
#[async_trait::async_trait]
impl flowtorrent::engine::TorrentEngine for FailFirstAdd {
    async fn inspect(&self, path: &std::path::Path) -> AppResult<TorrentPreview> {
        self.inner.inspect(path).await
    }
    async fn add(
        &mut self,
        preview: &TorrentPreview,
        request: &AddRequest,
        mode: AddMode,
    ) -> AppResult<TorrentId> {
        if std::mem::take(&mut self.fail) {
            return Err(AppError::DiskFull(request.base_dir.clone()));
        }
        self.inner.add(preview, request, mode).await
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
    async fn set_limits(&mut self, limits: TransferLimits) -> AppResult<()> {
        self.inner.set_limits(limits).await
    }
    async fn shutdown(&mut self) -> AppResult<()> {
        self.inner.shutdown().await
    }
}

#[tokio::test]
async fn one_failed_add_does_not_block_newer_torrents_and_can_be_retried() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = paths(root.path());
    std::fs::create_dir(&paths.download_dir)?;
    let engine = FailFirstAdd {
        inner: LibrqbitEngine::with_options(
            paths.download_dir.clone(),
            support::local_options(),
            vec![],
        )
        .await?,
        fail: true,
    };
    let store = Arc::new(MemoryStore::default());
    let mut handle = AppController::spawn(
        Box::new(engine),
        store.clone(),
        Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        )),
        paths,
    );
    let mut ids = Vec::new();
    for index in 0..2 {
        let path = root.path().join(format!("{index}.torrent"));
        std::fs::write(
            &path,
            support::torrent_bytes(&support::payload(), index == 1),
        )?;
        let item = preview(&mut handle, path).await?;
        ids.push(item.id);
        let folder = root.path().join(format!("target{index}"));
        std::fs::create_dir(&folder)?;
        handle
            .commands
            .send(AppCommand::ConfirmAdd(AddRequest::new(
                item.id,
                folder,
                item.default_selection(),
            )))
            .await?;
        until(&mut handle, |s| {
            s.torrents.len() == index + 1 && s.preview.is_none()
        })
        .await?;
    }
    let rows = handle.snapshots.borrow().torrents.clone();
    assert_eq!(
        rows.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![ids[1], ids[0]]
    );
    assert_eq!(rows[1].state, TransferState::Error);
    assert_eq!(rows[1].error.as_ref().unwrap().code, ErrorCode::DiskFull);
    assert_ne!(rows[0].state, TransferState::Error);
    assert_eq!(store.load()?.torrents.len(), 2);
    handle.commands.send(AppCommand::Retry(ids[0])).await?;
    until(&mut handle, |s| {
        s.torrents.len() == 2 && s.torrents.iter().all(|r| r.state != TransferState::Error)
    })
    .await?;
    stop(handle).await
}
