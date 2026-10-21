use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use async_trait::async_trait;
use librqbit::{
    AddTorrent, AddTorrentOptions, DhtSessionConfig, ListenerOptions, ManagedTorrent, Session,
    SessionOptions, TorrentStatsState,
};

use super::TorrentEngine;
use crate::{
    domain::*,
    error::{AppError, AppResult},
};

pub struct LibrqbitEngine {
    session: Arc<Session>,
    torrents: BTreeMap<TorrentId, Arc<ManagedTorrent>>,
    initial_peers: Vec<SocketAddr>,
}

impl LibrqbitEngine {
    pub async fn new(default_folder: PathBuf) -> AppResult<Self> {
        Self::with_options(
            default_folder,
            SessionOptions {
                dht: Some(DhtSessionConfig {
                    persistence: None,
                    ..Default::default()
                }),
                listen: Some(ListenerOptions::default()),
                client_name_and_version: Some(
                    concat!("FlowTorrent ", env!("CARGO_PKG_VERSION")).into(),
                ),
                ..Default::default()
            },
            vec![],
        )
        .await
    }

    /// Configure transport for controlled swarms without public discovery.
    /// Session auto-restore stays disabled regardless of caller options.
    pub async fn with_options(
        default_folder: PathBuf,
        mut options: SessionOptions,
        initial_peers: Vec<SocketAddr>,
    ) -> AppResult<Self> {
        options.persistence = None;
        options.fastresume = false;
        Ok(Self {
            session: Session::new_with_opts(default_folder, options).await?,
            torrents: BTreeMap::new(),
            initial_peers,
        })
    }

    fn handle(&self, id: TorrentId) -> AppResult<Arc<ManagedTorrent>> {
        self.torrents
            .get(&id)
            .cloned()
            .context("torrent is not registered")
            .map_err(AppError::from)
    }
}

#[async_trait]
impl TorrentEngine for LibrqbitEngine {
    async fn inspect(&self, path: &Path) -> AppResult<TorrentPreview> {
        let metadata = tokio::fs::read(path).await?;
        let parsed = librqbit::torrent_from_bytes(&metadata)
            .map_err(|e| AppError::InvalidMetadata(e.to_string()))?;
        let id = TorrentId(parsed.info_hash.0);
        let info = parsed
            .info
            .data
            .validate()
            .map_err(|e| AppError::InvalidMetadata(e.to_string()))?;
        let name = info
            .name()
            .map(|s| s.into_owned())
            .unwrap_or_else(|| "Torrent".into());
        let multi_file = info.info().files.is_some();
        let files = info
            .iter_file_details()
            .enumerate()
            .map(|(index, f)| TorrentFile {
                id: FileId(index),
                relative_path: f.filename.to_pathbuf(),
                length: f.len,
                is_padding: f.attrs().padding,
            })
            .collect();
        Ok(TorrentPreview {
            id,
            name,
            files,
            metadata,
            multi_file,
        })
    }

    async fn add(
        &mut self,
        preview: &TorrentPreview,
        request: &AddRequest,
        mode: AddMode,
    ) -> AppResult<TorrentId> {
        if request.id != preview.id
            || request.selected_files.is_empty()
            || request
                .selected_files
                .iter()
                .any(|id| !preview.files.iter().any(|f| f.id == *id && !f.is_padding))
        {
            return Err(AppError::InvalidMetadata("invalid file selection".into()));
        }
        if self.torrents.contains_key(&preview.id) {
            return Ok(preview.id);
        }
        let folder = if preview.multi_file {
            request.base_dir.join(&preview.name)
        } else {
            request.base_dir.clone()
        };
        let output_folder = folder
            .to_str()
            .context("download path is not valid Unicode")?
            .to_owned();
        let handle = self
            .session
            .add_torrent(
                AddTorrent::from_bytes(preview.metadata.clone()),
                Some(AddTorrentOptions {
                    output_folder: Some(output_folder),
                    only_files: Some(request.selected_files.iter().map(|f| f.0).collect()),
                    paused: mode == AddMode::RestorePaused,
                    overwrite: mode == AddMode::RestorePaused,
                    initial_peers: Some(self.initial_peers.clone()),
                    ..Default::default()
                }),
            )
            .await?
            .into_handle()
            .context("engine did not return a torrent handle")?;
        self.torrents.insert(preview.id, handle);
        Ok(preview.id)
    }

    async fn pause(&mut self, id: TorrentId) -> AppResult<()> {
        let handle = self.handle(id)?;
        self.session.pause(&handle).await?;
        Ok(())
    }

    async fn resume(&mut self, id: TorrentId) -> AppResult<()> {
        let handle = self.handle(id)?;
        self.session.unpause(&handle).await?;
        Ok(())
    }

    async fn forget(&mut self, id: TorrentId) -> AppResult<()> {
        let handle = self.handle(id)?;
        self.session.delete(handle.id().into(), false).await?;
        self.torrents.remove(&id);
        Ok(())
    }

    async fn snapshot(&self) -> AppResult<Vec<TorrentSnapshot>> {
        Ok(self
            .torrents
            .iter()
            .map(|(id, handle)| {
                let stats = handle.stats();
                let state = match stats.state {
                    TorrentStatsState::Initializing { .. } => TransferState::Checking,
                    TorrentStatsState::Error => TransferState::Error,
                    TorrentStatsState::Paused if stats.finished => TransferState::Completed,
                    TorrentStatsState::Paused => TransferState::Paused,
                    TorrentStatsState::Live if stats.finished => TransferState::Seeding,
                    TorrentStatsState::Live => {
                        if stats
                            .live
                            .as_ref()
                            .is_some_and(|s| s.snapshot.peer_stats.live > 0)
                        {
                            TransferState::Downloading
                        } else if stats
                            .live
                            .as_ref()
                            .is_some_and(|s| s.snapshot.peer_stats.connecting > 0)
                        {
                            TransferState::Connecting
                        } else {
                            TransferState::WaitingForSources
                        }
                    }
                };
                TorrentSnapshot {
                    id: *id,
                    name: handle.name().unwrap_or_default(),
                    selected_bytes: stats.total_bytes,
                    checked_downloaded_bytes: if state == TransferState::Checking {
                        0
                    } else {
                        stats.progress_bytes
                    },
                    download_bps: stats
                        .live
                        .as_ref()
                        .map(|s| s.download_speed.as_bytes())
                        .unwrap_or(0),
                    upload_bps: stats
                        .live
                        .as_ref()
                        .map(|s| s.upload_speed.as_bytes())
                        .unwrap_or(0),
                    state,
                    error: stats.error.map(|detail| UserError {
                        code: ErrorCode::Engine,
                        detail: Some(detail),
                    }),
                }
            })
            .collect())
    }

    async fn set_limits(&mut self, limits: TransferLimits) -> AppResult<()> {
        if [limits.download_bps, limits.upload_bps]
            .into_iter()
            .flatten()
            .any(|rate| rate.get() < MIN_LIMIT_BPS)
        {
            return Err(AppError::InvalidRate);
        }
        self.session
            .ratelimits
            .set_download_bps(limits.download_bps);
        self.session.ratelimits.set_upload_bps(limits.upload_bps);
        Ok(())
    }

    async fn shutdown(&mut self) -> AppResult<()> {
        self.session.stop().await;
        self.torrents.clear();
        Ok(())
    }
}
