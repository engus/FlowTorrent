use super::{
    commands::{AppCommand, AppOperation},
    preview::{PreviewQueue, PreviewSource},
    storage::SessionRepository,
    targets::{PreparedAdd, prepare_add},
};
use crate::{
    domain::*,
    engine::TorrentEngine,
    error::{AppError, AppResult},
    paths::AppPaths,
    platform::notifications::{CompletionNotice, completion_event},
    settings::{Settings, SettingsStore},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::sync::{mpsc, watch};

#[derive(Clone, Debug)]
pub struct AppSnapshot {
    pub torrents: Vec<TorrentSnapshot>,
    pub settings: Settings,
    pub visible: bool,
    pub preview: Option<TorrentPreview>,
    pub resolving_magnet: bool,
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
    Completion(CompletionNotice),
    ShutdownReady,
    CommandFinished {
        operation: AppOperation,
        success: bool,
    },
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
    magnet_job: Option<tokio::task::JoinHandle<AppResult<TorrentPreview>>>,
    claimed: Option<(AddRequest, Arc<PreparedAdd>)>,
    pending_adds: BTreeMap<TorrentId, (TorrentPreview, AddRequest)>,
    row_errors: BTreeMap<TorrentId, UserError>,
    completion_save_error: Option<UserError>,
    read_only: bool,
    checking: BTreeSet<TorrentId>,
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
            resolving_magnet: false,
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
            magnet_job: None,
            claimed: None,
            pending_adds: BTreeMap::new(),
            row_errors: BTreeMap::new(),
            completion_save_error: None,
            read_only: false,
            checking: BTreeSet::new(),
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
                    let operation=command.operation();
                    let success=if matches!(command,AppCommand::Shutdown) {
                        match self.engine.shutdown().await {
                            Ok(())=>{let _=self.events.send(AppEvent::ShutdownReady).await;break;}
                            Err(error)=>{self.report(None,error).await;false},
                        }
                    } else if self.read_only && !matches!(command, AppCommand::SetVisible(_)) {
                        false
                    } else {match self.command(command).await {Ok(())=>true,Err(error)=>{self.report(None,error).await;false}}};
                    self.publish().await;
                    let _=self.events.send(AppEvent::CommandFinished{operation,success}).await;
                }
                result = async { self.magnet_job.as_mut().unwrap().await }, if self.magnet_job.is_some() => {
                    self.magnet_job = None;
                    self.state.resolving_magnet = false;
                    let result = result.map_err(anyhow::Error::from).map_err(AppError::from).and_then(|result| result);
                    match result {
                        Ok(preview) => {
                            if !self.present_preview(preview).await {
                                self.show_next().await.ok();
                            }
                        }
                        Err(error) => {
                            self.queue.cancel_current();
                            self.report(None, error).await;
                            self.show_next().await.ok();
                        }
                    }
                    self.publish().await;
                }
                _=interval.tick()=>self.publish().await,
            }
        }
        self.cancel_magnet();
    }
    async fn initialize(&mut self) -> AppResult<()> {
        let store = self.store.clone();
        self.manifest = blocking(move || store.load()).await?;
        let settings_store = self.settings_store.clone();
        self.state.settings = blocking(move || settings_store.load()).await?;
        self.engine.set_limits(self.state.settings.limits).await?;
        let report =
            super::restore::restore_session(self.engine.as_mut(), &self.manifest, &self.paths)
                .await;
        self.checking = report.checking;
        self.row_errors = report.errors;
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
        if self.state.error.as_ref() == Some(&error)
            && id.is_none_or(|id| self.row_errors.get(&id) == Some(&error))
        {
            return;
        }
        tracing::warn!(torrent = ?id, code = ?error.code, detail = ?error.detail, "Application operation failed");
        if let Some(id) = id {
            self.row_errors.insert(id, error.clone());
        }
        self.state.error = Some(error.clone());
        let _ = self.events.send(AppEvent::Error { id, error }).await;
    }
    async fn show_next(&mut self) -> AppResult<()> {
        self.claimed = None;
        self.state.preview = None;
        while let Some(source) = self.queue.current().cloned() {
            let path = match source {
                PreviewSource::File(path) => path,
                PreviewSource::Magnet(link) => {
                    self.state.resolving_magnet = true;
                    self.magnet_job = Some(tokio::spawn(self.engine.inspect_magnet(link)));
                    break;
                }
            };
            match self.engine.inspect(&path).await {
                Ok(preview) => {
                    if !self.present_preview(preview).await {
                        continue;
                    }
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
    async fn present_preview(&mut self, preview: TorrentPreview) -> bool {
        if self
            .manifest
            .torrents
            .iter()
            .any(|record| record.id == preview.id)
        {
            let _ = self.events.send(AppEvent::FocusTorrent(preview.id)).await;
            self.queue.cancel_current();
            return false;
        }
        self.state.preview = Some(preview.clone());
        let _ = self.events.send(AppEvent::PreviewReady(preview)).await;
        true
    }
    fn cancel_magnet(&mut self) {
        if let Some(job) = self.magnet_job.take() {
            job.abort();
        }
        self.state.resolving_magnet = false;
    }
    async fn command(&mut self, command: AppCommand) -> AppResult<()> {
        if !matches!(command, AppCommand::SetVisible(_)) {
            self.state.error = None;
            self.completion_save_error = None;
        }
        match command {
            AppCommand::OpenMagnet(link) => {
                let link = crate::engine::magnet::normalize_magnet(&link)?;
                self.queue.enqueue_magnet(link);
                if self.state.preview.is_none() && self.magnet_job.is_none() {
                    self.show_next().await?;
                }
            }
            AppCommand::OpenTorrent(path) => {
                self.queue.enqueue(path);
                if self.state.preview.is_none() && self.magnet_job.is_none() {
                    self.show_next().await?;
                }
            }
            AppCommand::CancelPreview => {
                self.cancel_magnet();
                self.queue.cancel_current();
                self.show_next().await?;
            }
            AppCommand::ConfirmAdd(request) => self.confirm(request).await?,
            AppCommand::Pause(id) => {
                if !self.checking.contains(&id) {
                    self.engine.pause(id).await?;
                }
                let mut next = self.manifest.clone();
                record_mut(&mut next, id)?.desired_state = DesiredState::Paused;
                self.save(next).await?;
                self.row_errors.remove(&id);
            }
            AppCommand::Resume(id) => self.resume(id).await?,
            AppCommand::Retry(id) => {
                let result = self.retry(id).await;
                if let Err(error) = result {
                    self.row_errors.insert(id, error.user_error());
                    return Err(error);
                }
            }
            AppCommand::Forget(id) => {
                if self.engine.snapshot().await?.iter().any(|row| {
                    row.id == id
                        && !matches!(
                            row.state,
                            TransferState::Paused | TransferState::Completed | TransferState::Error
                        )
                }) {
                    self.engine.pause(id).await?;
                }
                let mut next = self.manifest.clone();
                next.torrents.retain(|r| r.id != id);
                self.save(next).await?;
                self.row_errors.remove(&id);
                self.pending_adds.remove(&id);
                self.checking.remove(&id);
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
        let cached = self
            .claimed
            .as_ref()
            .filter(|(previous, _)| {
                previous.id == request.id && previous.base_dir == request.base_dir
            })
            .map(|(_, prepared)| prepared.clone());
        let prepared = if let Some(prepared) = cached {
            prepared
        } else {
            let source = preview.clone();
            let pending = request.clone();
            let prepared = blocking(move || Ok(Arc::new(prepare_add(&source, &pending)?))).await?;
            self.claimed = Some((request.clone(), prepared.clone()));
            prepared
        };
        let selected = request.selected_files.clone();
        let mut request = blocking(move || prepared.confirm_selection(selected)).await?;
        // Hold the initial hash check paused, then apply the latest durable
        // intent once it finishes. Cancelling upstream initialization with
        // pause() can otherwise leave it indefinitely in Checking.
        request.start_paused = true;
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
                self.checking.insert(preview.id);
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
        if self.checking.contains(&id) {
            return Ok(());
        }
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
            self.checking.insert(id);
            self.row_errors.remove(&id);
            return Ok(());
        }
        let rows = self.engine.snapshot().await?;
        let existing = rows.iter().find(|row| row.id == id);
        if existing.is_none_or(|row| row.state == TransferState::Error) {
            let record = self
                .manifest
                .torrents
                .iter()
                .find(|record| record.id == id)
                .cloned()
                .ok_or_else(|| AppError::InvalidMetadata("Torrent is not registered".into()))?;
            if existing.is_some() {
                self.engine.forget(id).await?;
            }
            // Error recovery uses the same paused-first check as startup. Calling
            // upstream unpause on Error would start an untracked checksum task
            // which an immediate manual Pause can cancel indefinitely.
            if let Err(error) =
                super::restore::restore_record(self.engine.as_mut(), &record, &self.paths).await
            {
                self.row_errors.insert(id, error.user_error());
                return Err(error);
            }
            self.checking.insert(id);
            self.row_errors.remove(&id);
            return Ok(());
        }
        self.resume(id).await
    }
    async fn publish(&mut self) {
        let mut rows = match self.engine.snapshot().await {
            Ok(rows) => rows,
            Err(error) => {
                self.report(None, error).await;
                vec![]
            }
        };
        let mut changed = false;
        for id in self.checking.clone() {
            let Some(row) = rows.iter().find(|row| row.id == id) else {
                continue;
            };
            if row.state == TransferState::Checking {
                continue;
            }
            self.checking.remove(&id);
            if row.state == TransferState::Error {
                continue;
            }
            if self
                .manifest
                .torrents
                .iter()
                .any(|record| record.id == id && record.desired_state == DesiredState::Active)
            {
                match self.engine.resume(id).await {
                    Ok(()) => changed = true,
                    Err(error) => self.report(Some(id), error).await,
                }
            }
        }
        if changed {
            match self.engine.snapshot().await {
                Ok(updated) => rows = updated,
                Err(error) => self.report(None, error).await,
            }
        }
        let mut records: Vec<_> = self.manifest.torrents.iter().collect();
        records.sort_by_key(|r| std::cmp::Reverse(r.position));
        let previous = std::mem::take(&mut self.state.torrents);
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
                        destination: record.base_dir.clone(),
                        desired_state: record.desired_state,
                    });
                row.desired_state = record.desired_state;
                if let Some(error) = self.row_errors.get(&record.id) {
                    row.state = TransferState::Error;
                    row.error = Some(error.clone());
                }
                if !self.row_errors.contains_key(&record.id) && let Some(error) = &row.error
                    && !previous.iter().any(|previous| previous.id == row.id && previous.error.as_ref() == Some(error)) {
                    tracing::warn!(torrent = %row.id, code = ?error.code, detail = ?error.detail, "Torrent storage failed");
                }
                row
            })
            .collect();
        let mut next = self.manifest.clone();
        let mut notices = Vec::new();
        for record in &mut next.torrents {
            if let Some(row) = self.state.torrents.iter().find(|row| row.id == record.id)
                && let Some(notice) =
                    completion_event(record, row, self.state.settings.notify_on_complete)
            {
                notices.push(notice);
            }
        }
        if next
            .torrents
            .iter()
            .zip(&self.manifest.torrents)
            .any(|(next, previous)| next.completion_notified != previous.completion_notified)
        {
            match self.save(next).await {
                Ok(()) => {
                    if self
                        .completion_save_error
                        .take()
                        .is_some_and(|error| self.state.error.as_ref() == Some(&error))
                    {
                        self.state.error = None;
                    }
                    for notice in notices {
                        let _ = self.events.send(AppEvent::Completion(notice)).await;
                    }
                }
                Err(error) => {
                    self.completion_save_error = Some(error.user_error());
                    self.report(None, error).await;
                }
            }
        }
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
