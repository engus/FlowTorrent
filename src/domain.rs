use std::{num::NonZeroU32, path::PathBuf};

pub const MIN_LIMIT_BPS: u32 = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TorrentId(pub [u8; 20]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferState {
    Checking,
    Connecting,
    Downloading,
    WaitingForSources,
    Paused,
    Seeding,
    Completed,
    Error,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransferLimits {
    pub download_bps: Option<NonZeroU32>,
    pub upload_bps: Option<NonZeroU32>,
}

#[derive(Clone, Debug)]
pub struct TorrentFile {
    pub id: FileId,
    pub relative_path: PathBuf,
    pub length: u64,
    pub is_padding: bool,
}

#[derive(Clone, Debug)]
pub struct TorrentPreview {
    pub id: TorrentId,
    pub name: String,
    pub files: Vec<TorrentFile>,
    pub metadata: Vec<u8>,
    pub multi_file: bool,
}

#[derive(Clone, Debug)]
pub struct AddRequest {
    pub id: TorrentId,
    pub base_dir: PathBuf,
    pub selected_files: Vec<FileId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddMode {
    New,
    RestorePaused,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidMetadata,
    Engine,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserError {
    pub code: ErrorCode,
    pub detail: Option<String>,
}

#[derive(Clone, Debug)]
pub struct TorrentSnapshot {
    pub id: TorrentId,
    pub name: String,
    pub selected_bytes: u64,
    pub checked_downloaded_bytes: u64,
    pub download_bps: u64,
    pub upload_bps: u64,
    pub state: TransferState,
    pub error: Option<UserError>,
}
