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
use super::storage::SecureStorageFactory;
use crate::{
    app::targets::{prepare_add, prepare_restore},
    domain::*,
    error::{AppError, AppResult},
};
use librqbit::storage::StorageFactoryExt;
use tokio::io::AsyncReadExt;

fn parse_preview(metadata: Vec<u8>) -> AppResult<TorrentPreview> {
    use librqbit_bencode::BencodeValue;
    let raw: librqbit_bencode::BencodeValueBorrowed<'_> =
        librqbit_bencode::from_bytes(&metadata)
            .map_err(|error| AppError::InvalidMetadata(error.to_string()))?;
    if let BencodeValue::Dict(root) = &raw
        && let Some(BencodeValue::Dict(info)) = root
            .iter()
            .find(|(key, _)| key.as_ref() == b"info")
            .map(|(_, value)| value)
    {
        let v2 = info.iter().any(|(key, value)| {
            key.as_ref() == b"meta version" && matches!(value, BencodeValue::Integer(2))
        });
        if v2 && !info.keys().any(|key| key.as_ref() == b"pieces") {
            return Err(AppError::UnsupportedFormat);
        }
    }
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
        const MAX_METADATA: u64 = 16 * 1024 * 1024;
        let file = tokio::fs::File::open(path).await?;
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
                    paused: mode == AddMode::RestorePaused,
                    overwrite: mode == AddMode::RestorePaused,
                    initial_peers: Some(self.initial_peers.clone()),
                    storage_factory: Some(SecureStorageFactory::new(prepared).boxed()),
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
