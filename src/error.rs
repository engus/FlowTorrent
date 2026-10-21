#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("This torrent format is not supported")]
    UnsupportedFormat,
    #[error("A file already exists at {0}")]
    FileConflict(std::path::PathBuf),
    #[error("The disk is full at {0}")]
    DiskFull(std::path::PathBuf),
    #[error("Permission denied at {0}")]
    PermissionDenied(std::path::PathBuf),
    #[error("The download volume is unavailable at {0}")]
    MissingVolume(std::path::PathBuf),
    #[error("Saved data is damaged at {path}: {detail}")]
    CorruptState {
        path: std::path::PathBuf,
        detail: String,
    },
    #[error("Unsupported saved-data version {version} at {path}")]
    UnsupportedSchema {
        path: std::path::PathBuf,
        version: u64,
    },
    #[error("Folder is unavailable: {0}")]
    UnavailableFolder(std::path::PathBuf),
    #[error("Cannot discover the required operating system folders")]
    MissingSystemFolder,
    #[error("Cannot serialize saved data: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("The minimum transfer limit is 0.02 MB/s (20,000 bytes/s)")]
    InvalidRate,
    #[error("Invalid torrent metadata: {0}")]
    InvalidMetadata(String),
    #[error("I/O failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("Torrent engine failure: {0:#}")]
    Engine(#[from] anyhow::Error),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn user_error(&self) -> crate::domain::UserError {
        use crate::domain::{ErrorCode, UserError};
        let code = match self {
            Self::UnsupportedFormat => ErrorCode::UnsupportedFormat,
            Self::FileConflict(_) => ErrorCode::FileConflict,
            Self::DiskFull(_) => ErrorCode::DiskFull,
            Self::PermissionDenied(_) => ErrorCode::PermissionDenied,
            Self::MissingVolume(_) => ErrorCode::MissingVolume,
            Self::CorruptState { .. } | Self::UnsupportedSchema { .. } => ErrorCode::CorruptState,
            Self::UnavailableFolder(_) | Self::MissingSystemFolder => ErrorCode::UnavailableFolder,
            Self::InvalidRate => ErrorCode::InvalidRate,
            Self::InvalidMetadata(_) => ErrorCode::InvalidMetadata,
            Self::Io(error) => io_error_code(error).unwrap_or(ErrorCode::Engine),
            Self::Engine(error) => engine_error_code(error),
            _ => ErrorCode::Engine,
        };
        UserError {
            code,
            detail: Some(self.to_string()),
        }
    }
}
fn io_error_code(error: &std::io::Error) -> Option<crate::domain::ErrorCode> {
    use crate::domain::ErrorCode;
    match error.kind() {
        std::io::ErrorKind::StorageFull => Some(ErrorCode::DiskFull),
        std::io::ErrorKind::PermissionDenied => Some(ErrorCode::PermissionDenied),
        std::io::ErrorKind::NotFound => Some(ErrorCode::MissingVolume),
        _ => None,
    }
}
pub fn engine_error_code(error: &anyhow::Error) -> crate::domain::ErrorCode {
    error
        .chain()
        .find_map(|cause| {
            cause
                .downcast_ref::<std::io::Error>()
                .and_then(io_error_code)
        })
        .unwrap_or(crate::domain::ErrorCode::Engine)
}
