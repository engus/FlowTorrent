use crate::{
    domain::{AddRequest, TorrentId},
    settings::Settings,
};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum AppCommand {
    OpenTorrent(PathBuf),
    ConfirmAdd(AddRequest),
    CancelPreview,
    Pause(TorrentId),
    Resume(TorrentId),
    Forget(TorrentId),
    Retry(TorrentId),
    UpdateSettings(Settings),
    SetVisible(bool),
    Shutdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppOperation {
    OpenTorrent,
    ConfirmAdd,
    CancelPreview,
    Pause,
    Resume,
    Forget,
    Retry,
    UpdateSettings,
    SetVisible,
    Shutdown,
}
impl AppCommand {
    pub fn operation(&self) -> AppOperation {
        match self {
            Self::OpenTorrent(_) => AppOperation::OpenTorrent,
            Self::ConfirmAdd(_) => AppOperation::ConfirmAdd,
            Self::CancelPreview => AppOperation::CancelPreview,
            Self::Pause(_) => AppOperation::Pause,
            Self::Resume(_) => AppOperation::Resume,
            Self::Forget(_) => AppOperation::Forget,
            Self::Retry(_) => AppOperation::Retry,
            Self::UpdateSettings(_) => AppOperation::UpdateSettings,
            Self::SetVisible(_) => AppOperation::SetVisible,
            Self::Shutdown => AppOperation::Shutdown,
        }
    }
}
