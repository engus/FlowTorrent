use crate::error::{AppError, AppResult};
use directories::{ProjectDirs, UserDirs};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub config_file: PathBuf,
    pub data_dir: PathBuf,
    pub metadata_dir: PathBuf,
    pub manifest_file: PathBuf,
    pub download_dir: PathBuf,
}

impl AppPaths {
    /// Discover standard OS paths without creating folders or choosing a fallback.
    pub fn discover() -> AppResult<Self> {
        let project = ProjectDirs::from("com", "engus", "flowtorrent")
            .ok_or(AppError::MissingSystemFolder)?;
        let user = UserDirs::new().ok_or(AppError::MissingSystemFolder)?;
        let download_dir = user
            .download_dir()
            .ok_or(AppError::MissingSystemFolder)?
            .to_owned();
        let data_dir = project.data_local_dir().to_owned();
        Ok(Self {
            config_file: project.config_dir().join("settings.json"),
            metadata_dir: data_dir.join("metadata"),
            manifest_file: data_dir.join("session.json"),
            data_dir,
            download_dir,
        })
    }
}
