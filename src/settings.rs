use crate::{
    domain::TransferLimits,
    error::{AppError, AppResult},
    persistence::{atomic_write_json, check_schema},
};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub download_dir: PathBuf,
    pub notify_on_complete: bool,
    pub limits: TransferLimits,
}

impl Settings {
    pub fn defaults(download_dir: PathBuf) -> Self {
        Self {
            download_dir,
            notify_on_complete: true,
            limits: TransferLimits::default(),
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        self.limits.validate()?;
        if !self.download_dir.is_absolute() || !self.download_dir.is_dir() {
            return Err(AppError::UnavailableFolder(self.download_dir.clone()));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct SavedSettings {
    schema_version: u32,
    #[serde(flatten)]
    settings: Settings,
}

pub struct SettingsStore {
    path: PathBuf,
    default_download_dir: PathBuf,
}

impl SettingsStore {
    pub fn new(path: PathBuf, default_download_dir: PathBuf) -> Self {
        Self {
            path,
            default_download_dir,
        }
    }

    pub fn load(&self) -> AppResult<Settings> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Settings::defaults(self.default_download_dir.clone()));
            }
            Err(error) => return Err(error.into()),
        };
        check_schema(&self.path, &bytes)?;
        let saved: SavedSettings =
            serde_json::from_slice(&bytes).map_err(|error| AppError::CorruptState {
                path: self.path.clone(),
                detail: error.to_string(),
            })?;
        saved.settings.limits.validate()?;
        // An external download volume may currently be absent: retain its path
        // on load, then report the failure when a new download is confirmed.
        Ok(saved.settings)
    }

    pub fn save(&self, settings: &Settings) -> AppResult<()> {
        settings.validate()?;
        // Refuse to replace damaged or future-version data, including on Save.
        self.load()?;
        atomic_write_json(
            &self.path,
            &SavedSettings {
                schema_version: 1,
                settings: settings.clone(),
            },
        )
    }
}
