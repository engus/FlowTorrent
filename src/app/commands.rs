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
