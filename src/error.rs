#[derive(Debug, thiserror::Error)]
pub enum AppError {
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
