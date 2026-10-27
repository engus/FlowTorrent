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
    settings::{Settings, SettingsRecovery, SettingsStore},
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
    pub details: Option<Arc<TorrentDetails>>,
    pub settings: Settings,
    pub settings_recovery: Option<SettingsRecovery>,
    pub blocked_state_path: Option<PathBuf>,
    pub visible: bool,
    pub preview: Option<Arc<TorrentPreview>>,
    pub resolving_magnet: bool,
    pub error: Option<UserError>,
}
#[derive(Clone, Debug)]
pub enum AppEvent {
    PreviewReady(Arc<TorrentPreview>),
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
pub type TrashHandler = Arc<dyn Fn(&std::path::Path) -> AppResult<()> + Send + Sync>;
pub struct AppController {
    engine: Box<dyn TorrentEngine>,
    trash_handler: TrashHandler,
    login_provider: Arc<dyn crate::platform::login::LoginProvider>,
    store: Arc<dyn SessionRepository>,
    settings_store: Arc<SettingsStore>,
    paths: AppPaths,
    manifest: Manifest,
    state: AppSnapshot,
    queue: PreviewQueue,
    magnet_job: Option<tokio::task::JoinHandle<AppResult<TorrentPreview>>>,
    uncommitted: BTreeMap<(TorrentId, PathBuf), Arc<PreparedAdd>>,
    pending_adds: BTreeMap<TorrentId, (Arc<TorrentPreview>, AddRequest)>,
    row_errors: BTreeMap<TorrentId, UserError>,
    completion_save_error: Option<UserError>,
    read_only: bool,
    checking: BTreeSet<TorrentId>,
    detail_id: Option<TorrentId>,
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
        Self::spawn_with_trash(
            engine,
            store,
            settings_store,
            paths,
            Arc::new(crate::platform::trash::move_to_trash),
        )
    }
    /// Native-operation injection for controlled fixtures; production uses OS Trash.
    pub fn spawn_with_trash(
        engine: Box<dyn TorrentEngine>,
        store: Arc<dyn SessionRepository>,
        settings_store: Arc<SettingsStore>,
        paths: AppPaths,
        trash_handler: TrashHandler,
    ) -> AppHandle {
        Self::spawn_with_services(
            engine,
            store,
            settings_store,
            paths,
            trash_handler,
            Arc::new(crate::platform::login::NativeLogin),
        )
    }
    pub fn spawn_with_services(
        engine: Box<dyn TorrentEngine>,
        store: Arc<dyn SessionRepository>,
        settings_store: Arc<SettingsStore>,
        paths: AppPaths,
        trash_handler: TrashHandler,
        login_provider: Arc<dyn crate::platform::login::LoginProvider>,
    ) -> AppHandle {
        let state = AppSnapshot {
            torrents: vec![],
            details: None,
            settings: Settings::defaults(paths.download_dir.clone()),
            settings_recovery: None,
            blocked_state_path: None,
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
            trash_handler,
            login_provider,
            store,
            settings_store,
            paths,
            manifest: Manifest::default(),
            state,
            queue: PreviewQueue::default(),
            magnet_job: None,
            uncommitted: BTreeMap::new(),
            pending_adds: BTreeMap::new(),
            row_errors: BTreeMap::new(),
            completion_save_error: None,
            read_only: false,
            checking: BTreeSet::new(),
            detail_id: None,
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
            self.state.blocked_state_path = match &error {
                AppError::CorruptState { path, .. } | AppError::UnsupportedSchema { path, .. } => {
                    Some(path.clone())
                }
                _ => None,
            };
            self.report(None, error).await;
        }
        self.publish().await;
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                command = requests.recv() => {
                    let Some(command) = command else {
                        let _ = self.engine.shutdown().await;
                        break;
                    };
                    let operation = command.operation();
                    let success = if matches!(command, AppCommand::Shutdown) {
                        match self.engine.shutdown().await {
                            Ok(()) => {
                                let _ = self.events.send(AppEvent::ShutdownReady).await;
                                break;
                            }
                            Err(error) => {
                                self.report(None, error).await;
                                false
                            }
                        }
                    } else if self.read_only && !matches!(command, AppCommand::SetVisible(_)) {
                        false
                    } else {
                        match self.command(command).await {
                            Ok(()) => true,
                            Err(error) => {
                                self.report(None, error).await;
                                false
                            }
                        }
                    };
                    self.publish().await;
                    let _ = self.events.send(AppEvent::CommandFinished { operation, success }).await;
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
                _ = interval.tick() => self.publish().await,
            }
        }
        self.cancel_magnet();
    }
    async fn initialize(&mut self) -> AppResult<()> {
        // Preferences remain meaningful even when the registry cannot be restored.
        let settings_store = self.settings_store.clone();
        let loaded = blocking(move || settings_store.load_for_app()).await?;
        self.state.settings = loaded.settings;
        self.state.settings_recovery = loaded.recovery;
        let store = self.store.clone();
        self.manifest = blocking(move || store.load()).await?;
        self.engine.set_limits(self.state.settings.limits).await?;
        let mut restorable = self.manifest.clone();
        restorable
            .torrents
            .retain(|record| !self.trash_journal(record.id).try_exists().unwrap_or(true));
        let report =
            super::restore::restore_session(self.engine.as_mut(), &restorable, &self.paths).await;
        self.checking = report.checking;
        self.row_errors = report.errors;
        for record in &self.manifest.torrents {
            let journal = self.trash_journal(record.id);
            if journal.try_exists().unwrap_or(true) {
                self.row_errors.insert(
                    record.id,
                    AppError::TrashRecovery {
                        path: journal,
                        detail: "Interrupted operation; payload restore is blocked".into(),
                    }
                    .user_error(),
                );
            }
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
        let preview = Arc::new(preview);
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
                if let Err(error) = self.pause(id).await {
                    self.row_errors.insert(id, error.user_error());
                    return Err(error);
                }
            }
            AppCommand::Resume(id) => self.resume(id).await?,
            AppCommand::Retry(id) => {
                let result = self.retry(id).await;
                if let Err(error) = result {
                    self.row_errors.insert(id, error.user_error());
                    return Err(error);
                }
            }
            AppCommand::RemoveMany(ids, with_files) => {
                let mut first = None;
                for id in ids.into_iter().collect::<BTreeSet<_>>() {
                    let result = if with_files {
                        self.trash(id).await
                    } else {
                        self.forget(id).await
                    };
                    if let Err(error) = result {
                        self.row_errors.insert(id, error.user_error());
                        if first.is_none() {
                            first = Some(error);
                        }
                    }
                }
                if let Some(error) = first {
                    return Err(error);
                }
            }
            AppCommand::Trash(id) => {
                if let Err(error) = self.trash(id).await {
                    self.row_errors.insert(id, error.user_error());
                    return Err(error);
                }
            }
            AppCommand::Forget(id) => {
                if let Err(error) = self.forget(id).await {
                    self.row_errors.insert(id, error.user_error());
                    return Err(error);
                }
            }
            AppCommand::UpdateSettings(settings) => {
                settings.validate()?;
                self.engine.set_limits(settings.limits).await?;
                let login_changed =
                    settings.ui.launch_at_login != self.state.settings.ui.launch_at_login;
                let login_change = if login_changed {
                    let enabled = settings.ui.launch_at_login;
                    let provider = self.login_provider.clone();
                    match blocking(move || provider.apply(enabled)).await {
                        Ok(change) => Some(change),
                        Err(error) => {
                            self.engine.set_limits(self.state.settings.limits).await?;
                            return Err(error);
                        }
                    }
                } else {
                    None
                };
                let store = self.settings_store.clone();
                let saved = settings.clone();
                let recovery = match blocking(move || store.save_for_app(&saved)).await {
                    Ok(recovery) => recovery,
                    Err(error) => {
                        if let Some(change) = login_change
                            && let Err(rollback) = blocking(move || change.rollback()).await
                        {
                            tracing::error!(%rollback,"could not restore login entry after settings save failed");
                        }
                        self.engine.set_limits(self.state.settings.limits).await?;
                        return Err(error);
                    }
                };
                self.state.settings = settings;
                self.state.settings_recovery = recovery;
            }
            AppCommand::InspectDetails(id) => self.detail_id = id,
            AppCommand::SetAllActive(active) => {
                let ids = self.manifest.torrents.iter().map(|r| r.id).collect();
                self.set_active(ids, active).await?;
            }
            AppCommand::SetActive(ids, active) => self.set_active(ids, active).await?,
            AppCommand::SetVisible(visible) => self.state.visible = visible,
            AppCommand::Shutdown => unreachable!(),
        }
        Ok(())
    }
    async fn set_active(&mut self, ids: Vec<TorrentId>, active: bool) -> AppResult<()> {
        let mut first_error = None;
        for id in ids.into_iter().collect::<BTreeSet<_>>() {
            if active
                && (self.row_errors.contains_key(&id)
                    || self
                        .state
                        .torrents
                        .iter()
                        .any(|r| r.id == id && r.state == TransferState::Error))
            {
                continue;
            }
            let result = if active {
                self.resume(id).await
            } else {
                self.pause(id).await
            };
            if let Err(error) = result {
                self.row_errors.insert(id, error.user_error());
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(())
        }
    }
    async fn confirm(&mut self, request: AddRequest) -> AppResult<()> {
        if self.manifest.torrents.iter().any(|r| r.id == request.id) {
            let _ = self.events.send(AppEvent::FocusTorrent(request.id)).await;
            return Ok(());
        }
        let journal = self.trash_journal(request.id);
        if journal.try_exists()? {
            if super::trash::TrashStage::is_trashed(&journal, request.id)? {
                super::trash::TrashStage::finish_registry_removal(&journal, request.id)?;
            } else {
                return Err(AppError::TrashRecovery {
                    path: journal,
                    detail: "Resolve the previous Trash operation before adding this torrent again"
                        .into(),
                });
            }
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
        let claim_key = (request.id, request.base_dir.clone());
        let cached = self
            .uncommitted
            .get(&claim_key)
            .filter(|prepared| prepared.matches(&preview, &request))
            .cloned();
        let prepared = if let Some(prepared) = cached {
            let checked = prepared.clone();
            blocking(move || checked.validate_uncommitted()).await?;
            prepared
        } else {
            let source = preview.clone();
            let pending = request.clone();
            let prepared = blocking(move || Ok(Arc::new(prepare_add(&source, &pending)?))).await?;
            self.uncommitted.insert(claim_key.clone(), prepared.clone());
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
        self.uncommitted.remove(&claim_key);
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
    async fn pause(&mut self, id: TorrentId) -> AppResult<()> {
        let journal = self.trash_journal(id);
        if journal.try_exists()? {
            return Err(AppError::TrashRecovery {
                path: journal,
                detail: "Resolve the interrupted Trash operation first".into(),
            });
        }
        let mut next = self.manifest.clone();
        record_mut(&mut next, id)?.desired_state = DesiredState::Paused;
        self.save(next).await?;
        let actual = self
            .engine
            .snapshot()
            .await?
            .into_iter()
            .find(|row| row.id == id)
            .map(|row| row.state);
        if !self.checking.contains(&id)
            && actual.is_some_and(|state| {
                !matches!(
                    state,
                    TransferState::Paused | TransferState::Completed | TransferState::Error
                )
            })
        {
            self.engine.pause(id).await?;
        }
        if actual.is_some_and(|state| state != TransferState::Error) {
            self.row_errors.remove(&id);
        }
        Ok(())
    }
    fn trash_journal(&self, id: TorrentId) -> PathBuf {
        self.paths.data_dir.join("trash").join(format!("{id}.json"))
    }
    async fn trash(&mut self, id: TorrentId) -> AppResult<()> {
        let journal = self.trash_journal(id);
        let record = self
            .manifest
            .torrents
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .ok_or_else(|| AppError::InvalidMetadata("Torrent is not registered".into()))?;
        let recovery = |error: AppError| {
            if journal.try_exists().unwrap_or(true) {
                AppError::TrashRecovery {
                    path: journal.clone(),
                    detail: error.to_string(),
                }
            } else {
                error
            }
        };
        if journal.try_exists()? {
            let path = journal.clone();
            let handler = self.trash_handler.clone();
            blocking(move || {
                if super::trash::TrashStage::is_trashed(&path, id)? {
                    Ok(())
                } else {
                    super::trash::TrashStage::load(&path, id)?
                        .finish_with(move |path| handler(path))
                }
            })
            .await
            .map_err(recovery)?;
        } else {
            self.pause(id).await?;
            let prepared = self.engine.prepared_for_trash(id)?;
            self.engine.forget(id).await?;
            self.checking.remove(&id);
            let path = journal.clone();
            let handler = self.trash_handler.clone();
            blocking(move || {
                prepared
                    .stage_for_trash(id, &record.selected_files, &path)?
                    .finish_with(move |path| handler(path))
            })
            .await
            .map_err(recovery)?;
        }
        self.forget(id).await?;
        blocking(move || super::trash::TrashStage::finish_registry_removal(&journal, id)).await
    }
    async fn forget(&mut self, id: TorrentId) -> AppResult<()> {
        let metadata = self
            .manifest
            .torrents
            .iter()
            .find(|record| record.id == id)
            .ok_or_else(|| AppError::InvalidMetadata("Torrent is not registered".into()))?
            .metadata_path
            .clone();
        // Stop intent is durable first. Keep the record visible until the engine
        // has released it and the final registry save has actually succeeded.
        let mut stopped = self.manifest.clone();
        record_mut(&mut stopped, id)?.desired_state = DesiredState::Paused;
        self.save(stopped).await?;
        if let Some(row) = self
            .engine
            .snapshot()
            .await?
            .iter()
            .find(|row| row.id == id)
        {
            if !self.checking.contains(&id)
                && !matches!(
                    row.state,
                    TransferState::Paused | TransferState::Completed | TransferState::Error
                )
            {
                self.engine.pause(id).await?;
            }
            self.engine.forget(id).await?;
        }
        let mut next = self.manifest.clone();
        next.torrents.retain(|record| record.id != id);
        self.save(next).await?;
        self.row_errors.remove(&id);
        self.pending_adds.remove(&id);
        self.checking.remove(&id);
        // This cache is application-owned, never the user's source torrent or
        // downloaded payload. Preserve unexpected or damaged metainfo for repair.
        if self
            .engine
            .inspect(&metadata)
            .await
            .is_ok_and(|preview| preview.id == id)
            && let Err(error) = blocking(move || match std::fs::remove_file(metadata) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.into()),
            })
            .await
        {
            tracing::warn!(%error, "Could not remove obsolete metainfo cache");
        }
        Ok(())
    }
    async fn resume(&mut self, id: TorrentId) -> AppResult<()> {
        let journal = self.trash_journal(id);
        if journal.try_exists()? {
            return Err(AppError::TrashRecovery {
                path: journal,
                detail: "Resolve the interrupted Trash operation first".into(),
            });
        }
        let actual = self
            .engine
            .snapshot()
            .await?
            .into_iter()
            .find(|row| row.id == id)
            .map(|row| row.state);
        if actual == Some(TransferState::Error) {
            return Err(anyhow::anyhow!("Retry an errored torrent before resuming it").into());
        }
        let old = self.manifest.clone();
        let mut next = old.clone();
        record_mut(&mut next, id)?.desired_state = DesiredState::Active;
        self.save(next).await?;
        if self.checking.contains(&id) {
            return Ok(());
        }
        if actual.is_some_and(|state| {
            !matches!(
                state,
                TransferState::Paused | TransferState::Completed | TransferState::Checking
            )
        }) {
            self.row_errors.remove(&id);
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
        let journal = self.trash_journal(id);
        if journal.try_exists()? {
            return Err(AppError::TrashRecovery {
                path: journal,
                detail: "Resolve the interrupted Trash operation before restoring".into(),
            });
        }
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
        let desired_paused = self
            .manifest
            .torrents
            .iter()
            .any(|record| record.id == id && record.desired_state == DesiredState::Paused);
        // An earlier failed operation can leave the engine already satisfying
        // durable intent. librqbit rejects duplicate pause/resume calls.
        let actual = existing.expect("missing rows were restored above").state;
        let satisfied = if desired_paused {
            matches!(actual, TransferState::Paused | TransferState::Completed)
        } else {
            matches!(
                actual,
                TransferState::Connecting
                    | TransferState::Downloading
                    | TransferState::WaitingForSources
                    | TransferState::Seeding
            )
        };
        if satisfied || (actual == TransferState::Checking && self.checking.contains(&id)) {
            self.row_errors.remove(&id);
            return Ok(());
        }
        if desired_paused {
            self.pause(id).await
        } else {
            self.resume(id).await
        }
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
                        checking: None,
                        download_bps: 0,
                        upload_bps: 0,
        session_uploaded_bytes: 0,
                        state: TransferState::Error,
                        error: None,
                        destination: record.base_dir.clone(),
                        desired_state: record.desired_state,
                    });
                row.desired_state = record.desired_state;
                if let Some(error) = self.row_errors.get(&record.id) {
                    row.state = TransferState::Error;
                    row.error = Some(error.clone());
                    if error.code==ErrorCode::TrashRecovery && let Some(stage)=super::trash::TrashStage::recovery_path(&self.trash_journal(record.id)) {row.destination=stage;}
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
        self.state.details = if self.state.visible {
            if let Some(id) = self.detail_id {
                match self.engine.details(id).await {
                    Ok(Some(details)) => {
                        if self.state.details.as_deref() == Some(&details) {
                            self.state.details.clone()
                        } else {
                            Some(Arc::new(details))
                        }
                    }
                    Ok(None) => None,
                    Err(error) => {
                        self.report(Some(id), error).await;
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };
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
