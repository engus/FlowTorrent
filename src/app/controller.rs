use super::{
    commands::AppCommand,
    preview::PreviewQueue,
    storage::SessionRepository,
    targets::{PreparedAdd, prepare_add},
};
use crate::{
    domain::*,
    engine::TorrentEngine,
    error::{AppError, AppResult},
    paths::AppPaths,
    settings::{Settings, SettingsStore},
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{mpsc, watch};

#[derive(Clone, Debug)]
pub struct AppSnapshot {
    pub torrents: Vec<TorrentSnapshot>,
    pub settings: Settings,
    pub visible: bool,
    pub preview: Option<TorrentPreview>,
    pub error: Option<UserError>,
}
#[derive(Clone, Debug)]
pub enum AppEvent {
    PreviewReady(TorrentPreview),
    FocusTorrent(TorrentId),
    Error {
        id: Option<TorrentId>,
        error: UserError,
    },
    Completion(TorrentId),
    ShutdownReady,
}
pub struct AppHandle {
    pub commands: mpsc::Sender<AppCommand>,
    pub snapshots: watch::Receiver<AppSnapshot>,
    pub events: mpsc::Receiver<AppEvent>,
}
pub struct AppController {
    engine: Box<dyn TorrentEngine>,
    store: Arc<dyn SessionRepository>,
    settings_store: Arc<SettingsStore>,
    paths: AppPaths,
    manifest: Manifest,
    state: AppSnapshot,
    queue: PreviewQueue,
    claimed: Option<(AddRequest, Arc<PreparedAdd>)>,
    pending_adds: BTreeMap<TorrentId, (TorrentPreview, AddRequest)>,
    row_errors: BTreeMap<TorrentId, UserError>,
    read_only: bool,
    snapshots: watch::Sender<AppSnapshot>,
    events: mpsc::Sender<AppEvent>,
}

impl AppController {
    pub fn spawn(
        engine: Box<dyn TorrentEngine>,
        store: Arc<dyn SessionRepository>,
        settings_store: Arc<SettingsStore>,
        paths: AppPaths,
    ) -> AppHandle {
        let state = AppSnapshot {
            torrents: vec![],
            settings: Settings::defaults(paths.download_dir.clone()),
            visible: true,
            preview: None,
            error: None,
        };
        let (snapshots, receiver) = watch::channel(state.clone());
        let (commands, requests) = mpsc::channel(32);
        let (events, event_receiver) = mpsc::channel(64);
        let controller = Self {
            engine,
            store,
            settings_store,
            paths,
            manifest: Manifest::default(),
            state,
            queue: PreviewQueue::default(),
            claimed: None,
            pending_adds: BTreeMap::new(),
            row_errors: BTreeMap::new(),
            read_only: false,
            snapshots,
            events,
        };
        tokio::spawn(controller.run(requests));
        AppHandle {
            commands,
            snapshots: receiver,
            events: event_receiver,
        }
    }
    async fn run(mut self, mut requests: mpsc::Receiver<AppCommand>) {
        if let Err(error) = self.initialize().await {
            self.read_only = true;
            self.report(None, error).await;
        }
        self.publish().await;
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                command=requests.recv()=>{
                    let Some(command)=command else {let _=self.engine.shutdown().await;break;};
                    if matches!(command,AppCommand::Shutdown) {
                        match self.engine.shutdown().await {
                            Ok(())=>{let _=self.events.send(AppEvent::ShutdownReady).await;break;}
                            Err(error)=>self.report(None,error).await,
                        }
                    } else if let Err(error)=self.command(command).await {self.report(None,error).await;}
                    self.publish().await;
                }
                _=interval.tick()=>self.publish().await,
            }
        }
    }
    async fn initialize(&mut self) -> AppResult<()> {
        let store = self.store.clone();
        self.manifest = blocking(move || store.load()).await?;
        let settings_store = self.settings_store.clone();
        self.state.settings = blocking(move || settings_store.load()).await?;
        self.engine.set_limits(self.state.settings.limits).await?;
        // Persistent reconstruction is connected by the next plan stage.
        for record in &self.manifest.torrents {
            self.row_errors.insert(
                record.id,
                UserError {
                    code: ErrorCode::Engine,
                    detail: Some("Session restoration is pending".into()),
                },
            );
        }
        Ok(())
    }
    async fn save(&mut self, next: Manifest) -> AppResult<()> {
        let store = self.store.clone();
        let saved = next.clone();
        blocking(move || store.save(&saved)).await?;
        self.manifest = next;
        Ok(())
    }
    async fn report(&mut self, id: Option<TorrentId>, error: AppError) {
        let error = error.user_error();
        if let Some(id) = id {
            self.row_errors.insert(id, error.clone());
        }
        self.state.error = Some(error.clone());
        let _ = self.events.send(AppEvent::Error { id, error }).await;
    }
    async fn show_next(&mut self) -> AppResult<()> {
        self.claimed = None;
        self.state.preview = None;
        while let Some(path) = self.queue.current_path().map(ToOwned::to_owned) {
            match self.engine.inspect(&path).await {
                Ok(preview) => {
                    if self.manifest.torrents.iter().any(|r| r.id == preview.id) {
                        let _ = self.events.send(AppEvent::FocusTorrent(preview.id)).await;
                        self.queue.cancel_current();
                        continue;
                    }
                    self.state.preview = Some(preview.clone());
                    let _ = self.events.send(AppEvent::PreviewReady(preview)).await;
                    break;
                }
                Err(error) => {
                    self.queue.cancel_current();
                    self.report(None, error).await;
                }
            }
        }
        Ok(())
    }
    async fn command(&mut self, command: AppCommand) -> AppResult<()> {
        if self.read_only && !matches!(command, AppCommand::SetVisible(_)) {
            return Err(AppError::Engine(anyhow::anyhow!(
                "Saved state could not be loaded; restart after resolving the reported error"
            )));
        }
        self.state.error = None;
        match command {
            AppCommand::OpenTorrent(path) => {
                self.queue.enqueue(path);
                if self.state.preview.is_none() {
                    self.show_next().await?;
                }
            }
            AppCommand::CancelPreview => {
                self.queue.cancel_current();
                self.show_next().await?;
            }
            AppCommand::ConfirmAdd(request) => self.confirm(request).await?,
            AppCommand::Pause(id) => {
                self.engine.pause(id).await?;
                let mut next = self.manifest.clone();
                record_mut(&mut next, id)?.desired_state = DesiredState::Paused;
                self.save(next).await?;
                self.row_errors.remove(&id);
            }
            AppCommand::Resume(id) => self.resume(id).await?,
            AppCommand::Retry(id) => {
                let result = self.retry(id).await;
                if let Err(error) = result {
                    self.report(Some(id), error).await;
                }
            }
            AppCommand::Forget(id) => {
                if self.engine.snapshot().await?.iter().any(|row| row.id == id) {
                    self.engine.pause(id).await?;
                }
                let mut next = self.manifest.clone();
                next.torrents.retain(|r| r.id != id);
                self.save(next).await?;
                self.row_errors.remove(&id);
                self.pending_adds.remove(&id);
                if self.engine.snapshot().await?.iter().any(|row| row.id == id) {
                    self.engine.forget(id).await?;
                }
            }
            AppCommand::UpdateSettings(settings) => {
                settings.validate()?;
                self.engine.set_limits(settings.limits).await?;
                let store = self.settings_store.clone();
                let saved = settings.clone();
                if let Err(error) = blocking(move || store.save(&saved)).await {
                    self.engine.set_limits(self.state.settings.limits).await?;
                    return Err(error);
                }
                self.state.settings = settings;
            }
            AppCommand::SetVisible(visible) => self.state.visible = visible,
            AppCommand::Shutdown => unreachable!(),
        }
        Ok(())
    }
    async fn confirm(&mut self, request: AddRequest) -> AppResult<()> {
        if self.manifest.torrents.iter().any(|r| r.id == request.id) {
            let _ = self.events.send(AppEvent::FocusTorrent(request.id)).await;
            return Ok(());
        }
        let preview = self
            .state
            .preview
            .clone()
            .ok_or_else(|| AppError::InvalidMetadata("No pending torrent".into()))?;
        if request.id != preview.id {
            return Err(AppError::InvalidMetadata(
                "Confirmation does not match preview".into(),
            ));
        }
        let prepared = if let Some((previous, prepared)) = &self.claimed {
            if previous.id != request.id
                || previous.base_dir != request.base_dir
                || previous.selected_files != request.selected_files
            {
                return Err(AppError::InvalidMetadata(
                    "Retry the original confirmation before changing its destination or selection"
                        .into(),
                ));
            }
            prepared.clone()
        } else {
            let source = preview.clone();
            let pending = request.clone();
            let prepared = blocking(move || Ok(Arc::new(prepare_add(&source, &pending)?))).await?;
            self.claimed = Some((request.clone(), prepared.clone()));
            prepared
        };
        let request = blocking(move || prepared.confirm()).await?;
        let metadata_path = self
            .paths
            .metadata_dir
            .join(format!("{}.torrent", preview.id));
        let file = metadata_path.clone();
        let metadata = preview.metadata.clone();
        blocking(move || copy_metadata(file, metadata)).await?;
        let mut next = self.manifest.clone();
        let position = next.next_position;
        next.next_position = position
            .checked_add(1)
            .ok_or_else(|| AppError::InvalidMetadata("Session position overflow".into()))?;
        next.torrents.push(TorrentRecord {
            id: preview.id,
            name: preview.name.clone(),
            position,
            metadata_path,
            base_dir: request.base_dir.clone(),
            selected_files: request.selected_files.clone(),
            desired_state: DesiredState::Active,
            completion_notified: false,
        });
        self.save(next).await?;
        self.pending_adds
            .insert(preview.id, (preview.clone(), request.clone()));
        match self.engine.add(&preview, &request, AddMode::New).await {
            Ok(_) => {
                self.pending_adds.remove(&preview.id);
            }
            Err(error) => self.report(Some(preview.id), error).await,
        }
        self.queue.cancel_current();
        self.show_next().await?;
        Ok(())
    }
    async fn resume(&mut self, id: TorrentId) -> AppResult<()> {
        let old = self.manifest.clone();
        let mut next = old.clone();
        record_mut(&mut next, id)?.desired_state = DesiredState::Active;
        self.save(next).await?;
        if let Err(error) = self.engine.resume(id).await {
            self.save(old).await?;
            return Err(error);
        }
        self.row_errors.remove(&id);
        Ok(())
    }
    async fn retry(&mut self, id: TorrentId) -> AppResult<()> {
        if let Some((preview, request)) = self.pending_adds.get(&id).cloned() {
            self.engine.add(&preview, &request, AddMode::New).await?;
            self.pending_adds.remove(&id);
            self.row_errors.remove(&id);
            return Ok(());
        }
        self.resume(id).await
    }
    async fn publish(&mut self) {
        let rows = match self.engine.snapshot().await {
            Ok(rows) => rows,
            Err(error) => {
                self.report(None, error).await;
                vec![]
            }
        };
        let mut records: Vec<_> = self.manifest.torrents.iter().collect();
        records.sort_by_key(|r| std::cmp::Reverse(r.position));
        self.state.torrents = records
            .into_iter()
            .map(|record| {
                let mut row = rows
                    .iter()
                    .find(|row| row.id == record.id)
                    .cloned()
                    .unwrap_or_else(|| TorrentSnapshot {
                        id: record.id,
                        name: record.name.clone(),
                        selected_bytes: 0,
                        checked_downloaded_bytes: 0,
                        download_bps: 0,
                        upload_bps: 0,
                        state: TransferState::Error,
                        error: None,
                    });
                if let Some(error) = self.row_errors.get(&record.id) {
                    row.state = TransferState::Error;
                    row.error = Some(error.clone());
                }
                row
            })
            .collect();
        self.snapshots.send_replace(self.state.clone());
    }
}

fn record_mut(manifest: &mut Manifest, id: TorrentId) -> AppResult<&mut TorrentRecord> {
    manifest
        .torrents
        .iter_mut()
        .find(|r| r.id == id)
        .ok_or_else(|| AppError::InvalidMetadata("Torrent is not registered".into()))
}
async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(anyhow::Error::from)?
}
fn copy_metadata(path: PathBuf, bytes: Vec<u8>) -> AppResult<()> {
    use std::io::Write;
    if path.exists() {
        if std::fs::read(&path)? == bytes {
            return Ok(());
        }
        return Err(AppError::CorruptState {
            path,
            detail: "Saved metainfo differs from confirmed torrent".into(),
        });
    }
    let parent = path
        .parent()
        .ok_or_else(|| AppError::UnavailableFolder(path.clone()))?;
    std::fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(&bytes)?;
    temp.flush()?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(&path).map_err(|error| error.error)?;
    #[cfg(unix)]
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}
