pub mod librqbit;
mod storage;

use std::path::Path;

use async_trait::async_trait;

use crate::{domain::*, error::AppResult};

#[async_trait]
pub trait TorrentEngine: Send + Sync {
    async fn inspect(&self, path: &Path) -> AppResult<TorrentPreview>;
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
    async fn set_limits(&mut self, limits: TransferLimits) -> AppResult<()>;
    async fn shutdown(&mut self) -> AppResult<()>;
}
