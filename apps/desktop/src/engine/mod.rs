pub mod librqbit;
pub mod magnet;
mod metainfo;
mod storage;

use std::path::Path;

use async_trait::async_trait;

use crate::{domain::*, error::AppResult};

pub type MagnetInspection =
    std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<TorrentPreview>> + Send>>;

#[async_trait]
pub trait TorrentEngine: Send + Sync {
    async fn inspect(&self, path: &Path) -> AppResult<TorrentPreview>;
    /// An owned future lets metadata discovery run without borrowing the engine
    /// or blocking commands such as Cancel, Pause and Quit.
    fn inspect_magnet(&self, _link: String) -> MagnetInspection {
        Box::pin(async { Err(crate::error::AppError::UnsupportedFormat) })
    }
    async fn add(
        &mut self,
        preview: &TorrentPreview,
        request: &AddRequest,
        mode: AddMode,
    ) -> AppResult<TorrentId>;
    async fn pause(&mut self, id: TorrentId) -> AppResult<()>;
    async fn resume(&mut self, id: TorrentId) -> AppResult<()>;
    async fn forget(&mut self, id: TorrentId) -> AppResult<()>;
    async fn snapshot(&self) -> AppResult<Vec<TorrentSnapshot>>;
    fn prepared_for_trash(
        &self,
        _id: TorrentId,
    ) -> AppResult<std::sync::Arc<crate::app::targets::PreparedAdd>> {
        Err(crate::error::AppError::InvalidMetadata(
            "Torrent is unavailable for Trash".into(),
        ))
    }
    async fn details(&self, _id: TorrentId) -> AppResult<Option<TorrentDetails>> {
        Ok(None)
    }
    async fn set_limits(&mut self, limits: TransferLimits) -> AppResult<()>;
    async fn shutdown(&mut self) -> AppResult<()>;
}
