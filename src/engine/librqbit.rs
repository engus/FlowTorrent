use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use anyhow::Context;
use async_trait::async_trait;
use librqbit::{
    AddTorrent, AddTorrentOptions, DhtSessionConfig, ListenerOptions, ManagedTorrent, Session,
    SessionOptions, TorrentStatsState,
};

use super::TorrentEngine;
use super::storage::SecureStorageFactory;
use crate::{
    app::targets::{PreparedAdd, prepare_add, prepare_restore},
    domain::*,
    error::{AppError, AppResult},
};
use librqbit::storage::StorageFactoryExt;
use tokio::io::AsyncReadExt;

#[cfg(test)]
#[path = "fault_tests.rs"]
mod fault_tests;

fn parse_preview(metadata: Vec<u8>) -> AppResult<TorrentPreview> {
    super::metainfo::check_supported_format(&metadata)?;
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
    let mut files = Vec::new();
    for (index, file) in info.iter_file_details().enumerate() {
        if file.attrs().symlink {
            return Err(AppError::InvalidMetadata(
                "symlink entries are unsupported".into(),
            ));
        }
        files.push(TorrentFile {
            id: FileId(index),
            relative_path: file.filename.to_pathbuf(),
            length: file.len,
            is_padding: file.attrs().padding,
        });
    }
    Ok(TorrentPreview {
        id,
        name,
        files,
        metadata,
        multi_file,
    })
}

pub struct LibrqbitEngine {
    session: Arc<Session>,
    torrents: BTreeMap<TorrentId, EngineTorrent>,
    initial_peers: Vec<SocketAddr>,
}

struct EngineTorrent {
    handle: Arc<ManagedTorrent>,
    prepared: Arc<PreparedAdd>,
    readers_only: AtomicBool,
    uploaded_before_pause: AtomicU64,
}
impl EngineTorrent {
    fn release_writers(&self) {
        if !self.readers_only.load(Ordering::Relaxed) {
            match self.prepared.release_writers() {
                Ok(()) => self.readers_only.store(true, Ordering::Relaxed),
                Err(error) => tracing::warn!("could not release payload write access: {error}"),
            }
        }
    }
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
        #[cfg(unix)]
        {
            // Capability storage owns its payload handles. Use the engine's
            // supported, hard-limit-aware helper before opening the session.
            static NOFILE_INIT: std::sync::Once = std::sync::Once::new();
            NOFILE_INIT.call_once(|| {
                if let Err(error) = librqbit::try_increase_nofile_limit() {
                    tracing::warn!(%error, "Could not increase the process open-file limit");
                }
            });
        }
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
            .map(|torrent| torrent.handle.clone())
            .context("torrent is not registered")
            .map_err(AppError::from)
    }
}

#[async_trait]
impl TorrentEngine for LibrqbitEngine {
    fn inspect_magnet(&self, link: String) -> super::MagnetInspection {
        let session = self.session.clone();
        let initial_peers = self.initial_peers.clone();
        Box::pin(async move {
            let link = super::magnet::normalize_magnet(&link)?;
            let response = tokio::time::timeout(
                std::time::Duration::from_secs(120),
                session.add_torrent(
                    AddTorrent::from_url(link),
                    Some(AddTorrentOptions {
                        list_only: true,
                        initial_peers: Some(initial_peers),
                        ..Default::default()
                    }),
                ),
            )
            .await
            .map_err(|_| AppError::Magnet("Metadata lookup timed out".into()))?
            .map_err(|error| AppError::Magnet(error.to_string()))?;
            let librqbit::AddTorrentResponse::ListOnly(response) = response else {
                return Err(AppError::Magnet("Expected metadata-only response".into()));
            };
            if response.torrent_bytes.len() > 16 * 1024 * 1024 {
                return Err(AppError::Magnet("Torrent metadata exceeds 16 MiB".into()));
            }
            parse_preview(response.torrent_bytes.to_vec())
        })
    }

    async fn inspect(&self, path: &Path) -> AppResult<TorrentPreview> {
        const MAX_METADATA: u64 = 16 * 1024 * 1024;
        let file = tokio::fs::File::open(path).await.map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                AppError::InvalidMetadata(format!(
                    "Torrent file is unavailable: {}",
                    path.display()
                ))
            } else {
                AppError::Io(error)
            }
        })?;
        let mut metadata = Vec::new();
        file.take(MAX_METADATA + 1)
            .read_to_end(&mut metadata)
            .await?;
        if metadata.len() as u64 > MAX_METADATA {
            return Err(AppError::InvalidMetadata(
                "torrent metadata exceeds 16 MiB".into(),
            ));
        }
        parse_preview(metadata)
    }

    async fn add(
        &mut self,
        preview: &TorrentPreview,
        request: &AddRequest,
        mode: AddMode,
    ) -> AppResult<TorrentId> {
        if self.torrents.contains_key(&preview.id) {
            return Ok(preview.id);
        }
        let decoded = parse_preview(preview.metadata.clone())?;
        if decoded.id != preview.id
            || decoded.name != preview.name
            || decoded.files != preview.files
            || decoded.multi_file != preview.multi_file
        {
            return Err(AppError::InvalidMetadata(
                "preview differs from original metadata".into(),
            ));
        }
        let prepared = if let Some(prepared) = &request.prepared {
            if mode != AddMode::New || !prepared.matches(preview, request) {
                return Err(AppError::InvalidMetadata(
                    "prepared request differs from confirmation".into(),
                ));
            }
            prepared.clone()
        } else {
            let preview = preview.clone();
            let request = request.clone();
            tokio::task::spawn_blocking(move || {
                let prepared = Arc::new(if mode == AddMode::New {
                    prepare_add(&preview, &request)?
                } else {
                    prepare_restore(&preview, &request)?
                });
                prepared.claim_files()?;
                Ok::<_, AppError>(prepared)
            })
            .await
            .map_err(anyhow::Error::from)??
        };
        let output_folder = prepared
            .destination
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
                    paused: mode == AddMode::RestorePaused || request.start_paused,
                    overwrite: mode == AddMode::RestorePaused,
                    initial_peers: Some(self.initial_peers.clone()),
                    storage_factory: Some(SecureStorageFactory::new(prepared.clone()).boxed()),
                    ..Default::default()
                }),
            )
            .await?
            .into_handle()
            .context("engine did not return a torrent handle")?;
        self.torrents.insert(
            preview.id,
            EngineTorrent {
                handle,
                prepared,
                readers_only: AtomicBool::new(false),
                uploaded_before_pause: AtomicU64::new(0),
            },
        );
        Ok(preview.id)
    }

    async fn pause(&mut self, id: TorrentId) -> AppResult<()> {
        let handle = self.handle(id)?;
        let uploaded = handle.stats().uploaded_bytes;
        self.session.pause(&handle).await?;
        if let Some(torrent) = self.torrents.get(&id) {
            torrent
                .uploaded_before_pause
                .fetch_add(uploaded, Ordering::Relaxed);
        }
        if let Some(torrent) = self.torrents.get(&id) {
            torrent.release_writers();
        }
        Ok(())
    }

    async fn resume(&mut self, id: TorrentId) -> AppResult<()> {
        let handle = self.handle(id)?;
        self.session.unpause(&handle).await?;
        if let Some(torrent) = self.torrents.get(&id) {
            torrent.readers_only.store(false, Ordering::Relaxed);
        }
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
            .map(|(id, torrent)| {
                let handle = &torrent.handle;
                let stats = handle.stats();
                // Upstream totals count whole selected pieces, including bytes
                // belonging to neighbouring, unselected files. The UI reports
                // only the selected files and their verified per-file progress.
                let selected = handle.only_files();
                let (selected_bytes, checked_bytes) = torrent
                    .prepared
                    .source_files
                    .iter()
                    .filter(|file| {
                        !file.is_padding
                            && selected.as_ref().is_none_or(|ids| ids.contains(&file.id.0))
                    })
                    .fold((0, 0), |(total, checked), file| {
                        (
                            total + file.length,
                            checked
                                + stats
                                    .file_progress
                                    .get(file.id.0)
                                    .copied()
                                    .unwrap_or(0)
                                    .min(file.length),
                        )
                    });
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
                if matches!(
                    state,
                    TransferState::Seeding | TransferState::Completed | TransferState::Paused
                ) {
                    torrent.release_writers();
                }
                let error = handle
                    .with_state(|state| match state {
                        librqbit::ManagedTorrentState::Error(error) => Some(UserError {
                            code: crate::error::engine_error_code(error),
                            detail: Some(format!("{error:#}")),
                        }),
                        _ => None,
                    })
                    .or_else(|| {
                        stats.error.map(|detail| UserError {
                            code: ErrorCode::Engine,
                            detail: Some(detail),
                        })
                    });
                TorrentSnapshot {
                    id: *id,
                    name: handle.name().unwrap_or_default(),
                    selected_bytes,
                    checked_downloaded_bytes: if state == TransferState::Checking {
                        0
                    } else {
                        checked_bytes
                    },
                    download_bps: stats
                        .live
                        .as_ref()
                        .map(|s| s.download_speed.as_bytes())
                        .unwrap_or(0),
                    session_uploaded_bytes: torrent
                        .uploaded_before_pause
                        .load(Ordering::Relaxed)
                        .saturating_add(stats.uploaded_bytes),
                    upload_bps: stats
                        .live
                        .as_ref()
                        .map(|s| s.upload_speed.as_bytes())
                        .unwrap_or(0),
                    state,
                    error,
                    destination: handle.output_folder().to_owned(),
                    desired_state: if handle.is_paused() {
                        DesiredState::Paused
                    } else {
                        DesiredState::Active
                    },
                }
            })
            .collect())
    }

    async fn details(&self, id: TorrentId) -> AppResult<Option<TorrentDetails>> {
        let Some(torrent) = self.torrents.get(&id) else {
            return Ok(None);
        };
        let stats = torrent.handle.stats();
        let selected = torrent.handle.only_files();
        Ok(Some(TorrentDetails {
            id,
            files: torrent
                .prepared
                .source_files
                .iter()
                .filter(|file| !file.is_padding)
                .map(|file| FileProgress {
                    id: file.id,
                    path: file.relative_path.clone(),
                    length: file.length,
                    checked_bytes: if matches!(stats.state, TorrentStatsState::Initializing { .. })
                    {
                        0
                    } else {
                        stats
                            .file_progress
                            .get(file.id.0)
                            .copied()
                            .unwrap_or(0)
                            .min(file.length)
                    },
                    selected: selected.as_ref().is_none_or(|ids| ids.contains(&file.id.0)),
                })
                .collect(),
        }))
    }

    async fn set_limits(&mut self, limits: TransferLimits) -> AppResult<()> {
        limits.validate()?;
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
