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
    #[serde(default)]
    pub ui: UiPreferences,
}

impl Settings {
    pub fn defaults(download_dir: PathBuf) -> Self {
        Self {
            download_dir,
            notify_on_complete: true,
            limits: TransferLimits::default(),
            ui: UiPreferences::default(),
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguagePreference {
    #[default]
    System,
    Ru,
    En,
    Es,
    Pt,
    Fr,
    De,
    It,
    Pl,
    Tr,
    Ja,
    Ko,
    ZhCn,
    Kk,
    Uk,
}
impl LanguagePreference {
    pub const ALL: [Self; 15] = [
        Self::System,
        Self::Ru,
        Self::En,
        Self::Es,
        Self::Pt,
        Self::Fr,
        Self::De,
        Self::It,
        Self::Pl,
        Self::Tr,
        Self::Ja,
        Self::Ko,
        Self::ZhCn,
        Self::Kk,
        Self::Uk,
    ];
    pub fn choice(self) -> i32 {
        Self::ALL
            .iter()
            .position(|p| *p == self)
            .unwrap_or_default() as i32
    }
    pub fn from_choice(index: i32) -> Option<Self> {
        Self::ALL.get(usize::try_from(index).ok()?).copied()
    }
    pub fn tag(self) -> Option<&'static str> {
        Some(match self {
            Self::System => return None,
            Self::Ru => "ru",
            Self::En => "en",
            Self::Es => "es",
            Self::Pt => "pt",
            Self::Fr => "fr",
            Self::De => "de",
            Self::It => "it",
            Self::Pl => "pl",
            Self::Tr => "tr",
            Self::Ja => "ja",
            Self::Ko => "ko",
            Self::ZhCn => "zh-CN",
            Self::Kk => "kk",
            Self::Uk => "uk",
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPreferences {
    pub theme: ThemePreference,
    pub language: LanguagePreference,
    pub launch_at_login: bool,
    pub show_tray_icon: bool,
    pub layout: UiLayout,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            theme: ThemePreference::default(),
            language: LanguagePreference::default(),
            launch_at_login: false,
            show_tray_icon: true,
            layout: UiLayout::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiLayout {
    pub sidebar_width: u16,
    pub sidebar_collapsed: bool,
    pub drawer_height: u16,
}
impl Default for UiLayout {
    fn default() -> Self {
        Self {
            sidebar_width: 180,
            sidebar_collapsed: false,
            drawer_height: 320,
        }
    }
}
impl UiLayout {
    pub fn normalized(self) -> Self {
        Self {
            sidebar_width: self.sidebar_width.clamp(158, 280),
            drawer_height: self.drawer_height.clamp(220, 540),
            ..self
        }
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

#[derive(Clone, Debug)]
pub struct SettingsRecovery {
    pub original: PathBuf,
    pub replacement: PathBuf,
    pub status: RecoveryStatus,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryStatus {
    Defaults,
    Recovered,
    Blocked,
}
pub struct LoadedSettings {
    pub settings: Settings,
    pub recovery: Option<SettingsRecovery>,
}

impl SettingsStore {
    pub fn load_for_app(&self) -> AppResult<LoadedSettings> {
        match self.load() {
            Ok(settings) => Ok(LoadedSettings {
                settings,
                recovery: None,
            }),
            Err(_) => {
                let replacement = self.recovery_store();
                let (settings, status) = match replacement.path.try_exists() {
                    Ok(false) => (
                        Settings::defaults(self.default_download_dir.clone()),
                        RecoveryStatus::Defaults,
                    ),
                    Ok(true) => match replacement.load() {
                        Ok(settings) => (settings, RecoveryStatus::Recovered),
                        Err(_) => (
                            Settings::defaults(self.default_download_dir.clone()),
                            RecoveryStatus::Blocked,
                        ),
                    },
                    Err(_) => (
                        Settings::defaults(self.default_download_dir.clone()),
                        RecoveryStatus::Blocked,
                    ),
                };
                Ok(LoadedSettings {
                    settings,
                    recovery: Some(SettingsRecovery {
                        original: self.path.clone(),
                        replacement: replacement.path,
                        status,
                    }),
                })
            }
        }
    }
    pub fn save_for_app(&self, settings: &Settings) -> AppResult<Option<SettingsRecovery>> {
        if self.load().is_ok() {
            self.save(settings)?;
            return Ok(None);
        }
        let replacement = self.recovery_store();
        // The strict store protects an existing damaged/future recovery file too.
        replacement.save(settings)?;
        Ok(Some(SettingsRecovery {
            original: self.path.clone(),
            replacement: replacement.path,
            status: RecoveryStatus::Recovered,
        }))
    }
    fn recovery_store(&self) -> Self {
        Self::new(
            self.path.with_extension("recovered.json"),
            self.default_download_dir.clone(),
        )
    }
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
        saved
            .settings
            .limits
            .validate()
            .map_err(|error| AppError::CorruptState {
                path: self.path.clone(),
                detail: error.to_string(),
            })?;
        if !saved.settings.download_dir.is_absolute() {
            return Err(AppError::CorruptState {
                path: self.path.clone(),
                detail: "Download folder must be absolute".into(),
            });
        }
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
