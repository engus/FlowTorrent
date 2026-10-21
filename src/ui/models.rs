use super::i18n::{Locale, error_message, format_bytes, state_label};
use crate::domain::*;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{unc}");
    }
    if let Some(disk) = text.strip_prefix(r"\\?\")
        && disk.as_bytes().get(1) == Some(&b':')
    {
        return disk.to_owned();
    }
    text.into_owned()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorrentRowModel {
    pub id: TorrentId,
    pub name: String,
    pub destination: PathBuf,
    pub progress_permille: u16,
    pub volume: String,
    pub status: String,
    pub download: String,
    pub upload: String,
    pub action: String,
    pub error: Option<String>,
    pub can_retry: bool,
}
pub fn to_row(row: &TorrentSnapshot, locale: Locale) -> TorrentRowModel {
    let downloaded = row.checked_downloaded_bytes.min(row.selected_bytes);
    let progress_permille = if row.selected_bytes == 0 {
        0
    } else {
        (u128::from(downloaded) * 1000 / u128::from(row.selected_bytes)) as u16
    };
    let suffix = locale.text("/с", "/s");
    TorrentRowModel {
        id: row.id,
        name: row.name.clone(),
        destination: row.destination.clone(),
        progress_permille,
        volume: format!(
            "{} / {}",
            format_bytes(downloaded, locale),
            format_bytes(row.selected_bytes, locale)
        ),
        status: state_label(row.state, locale).into(),
        download: format!("{}{suffix}", format_bytes(row.download_bps, locale)),
        upload: format!("{}{suffix}", format_bytes(row.upload_bps, locale)),
        action: if row.state == TransferState::Error {
            locale.text("Повторить", "Retry")
        } else if row.desired_state == DesiredState::Paused {
            locale.text("Продолжить", "Resume")
        } else {
            locale.text("Пауза", "Pause")
        }
        .into(),
        error: row
            .error
            .as_ref()
            .map(|error| error_message(&error.code, locale).into()),
        can_retry: row.state == TransferState::Error,
    }
}

pub struct PreviewSelection {
    files: Vec<TorrentFile>,
    selected: BTreeSet<usize>,
}
impl PreviewSelection {
    pub fn new(preview: &TorrentPreview) -> Self {
        let files: Vec<_> = preview
            .files
            .iter()
            .filter(|file| !file.is_padding)
            .cloned()
            .collect();
        let selected = files.iter().map(|file| file.id.0).collect();
        Self { files, selected }
    }
    pub fn files(&self) -> &[TorrentFile] {
        &self.files
    }
    pub fn set_checked(&mut self, id: FileId, checked: bool) {
        if !self.files.iter().any(|file| file.id == id) {
            return;
        }
        if checked {
            self.selected.insert(id.0);
        } else {
            self.selected.remove(&id.0);
        }
    }
    pub fn selected_files(&self) -> Vec<FileId> {
        self.selected.iter().copied().map(FileId).collect()
    }
    pub fn selected_bytes(&self) -> u64 {
        self.files
            .iter()
            .filter(|file| self.selected.contains(&file.id.0))
            .fold(0, |sum, file| sum.saturating_add(file.length))
    }
    pub fn can_confirm(&self) -> bool {
        !self.selected.is_empty()
    }
}
