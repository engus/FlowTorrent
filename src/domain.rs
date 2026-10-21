use serde::{Deserialize, Serialize};
use std::{num::NonZeroU32, path::PathBuf};

pub const MIN_LIMIT_BPS: u32 = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TorrentId(pub [u8; 20]);

impl std::fmt::Display for TorrentId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl Serialize for TorrentId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for TorrentId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.len() != 40 {
            return Err(serde::de::Error::custom(
                "infohash must contain 40 hexadecimal characters",
            ));
        }
        fn hex(byte: u8) -> Option<u8> {
            match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                b'A'..=b'F' => Some(byte - b'A' + 10),
                _ => None,
            }
        }
        let mut id = [0; 20];
        for (slot, pair) in id.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
            let (Some(high), Some(low)) = (hex(pair[0]), hex(pair[1])) else {
                return Err(serde::de::Error::custom("invalid hexadecimal infohash"));
            };
            *slot = high * 16 + low;
        }
        Ok(Self(id))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FileId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesiredState {
    Active,
    Paused,
}

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferLimits {
    pub download_bps: Option<NonZeroU32>,
    pub upload_bps: Option<NonZeroU32>,
}

impl TransferLimits {
    pub fn validate(&self) -> crate::error::AppResult<()> {
        if [self.download_bps, self.upload_bps]
            .into_iter()
            .flatten()
            .any(|rate| rate.get() < MIN_LIMIT_BPS)
        {
            return Err(crate::error::AppError::InvalidRate);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TorrentRecord {
    pub id: TorrentId,
    pub name: String,
    pub position: u64,
    pub metadata_path: PathBuf,
    pub base_dir: PathBuf,
    pub selected_files: Vec<FileId>,
    pub desired_state: DesiredState,
    pub completion_notified: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub next_position: u64,
    pub torrents: Vec<TorrentRecord>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            schema_version: 1,
            next_position: 0,
            torrents: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

impl TorrentPreview {
    pub fn default_selection(&self) -> Vec<FileId> {
        self.files
            .iter()
            .filter(|file| !file.is_padding)
            .map(|file| file.id)
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct AddRequest {
    pub id: TorrentId,
    pub base_dir: PathBuf,
    pub selected_files: Vec<FileId>,
    pub(crate) prepared: Option<std::sync::Arc<crate::app::targets::PreparedAdd>>,
    pub(crate) start_paused: bool,
}

impl AddRequest {
    pub fn new(id: TorrentId, base_dir: PathBuf, selected_files: Vec<FileId>) -> Self {
        Self {
            id,
            base_dir,
            selected_files,
            prepared: None,
            start_paused: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddMode {
    New,
    RestorePaused,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidMetadata,
    UnsupportedFormat,
    FileConflict,
    UnavailableFolder,
    DiskFull,
    PermissionDenied,
    MissingVolume,
    CorruptState,
    InvalidRate,
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
    pub destination: PathBuf,
    pub desired_state: DesiredState,
}
