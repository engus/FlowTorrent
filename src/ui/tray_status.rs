use super::i18n::{Locale, format_bytes};
use crate::domain::{TorrentSnapshot, TransferState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayStatus {
    pub download_bps: u64,
    pub upload_bps: u64,
    pub downloading: usize,
    pub download: String,
    pub upload: String,
    pub active: String,
    pub tooltip: String,
    pub summary: String,
}
impl TrayStatus {
    pub fn from_rows(rows: &[TorrentSnapshot], locale: Locale) -> Self {
        let (download_bps, upload_bps, downloading) = totals(rows);
        let suffix = locale.text("/с", "/s");
        let download = format!(
            "↓ {}: {}{suffix}",
            locale.text("Скачивание", "Download"),
            format_bytes(download_bps, locale)
        );
        let upload = format!(
            "↑ {}: {}{suffix}",
            locale.text("Отдача", "Upload"),
            format_bytes(upload_bps, locale)
        );
        let active = format!(
            "{}: {downloading}",
            locale.text("Скачиваются", "Downloading")
        );
        let summary = format!(
            "↓ {}{suffix} · ↑ {}{suffix} · {active}",
            format_bytes(download_bps, locale),
            format_bytes(upload_bps, locale),
        );
        let tooltip = format!("FlowTorrent\n{download}\n{upload}\n{active}");
        Self {
            download_bps,
            upload_bps,
            downloading,
            download,
            upload,
            active,
            tooltip,
            summary,
        }
    }
}

fn totals(rows: &[TorrentSnapshot]) -> (u64, u64, usize) {
    let download_bps = rows
        .iter()
        .fold(0u64, |sum, row| sum.saturating_add(row.download_bps));
    let upload_bps = rows
        .iter()
        .fold(0u64, |sum, row| sum.saturating_add(row.upload_bps));
    let downloading = rows
        .iter()
        .filter(|row| row.state == TransferState::Downloading)
        .count();
    (download_bps, upload_bps, downloading)
}
#[derive(Default)]
pub struct TrayRefresh {
    last: Option<(u64, u64, usize, Locale, bool)>,
}
impl TrayRefresh {
    pub fn changed(&mut self, rows: &[TorrentSnapshot], locale: Locale, enabled: bool) -> bool {
        let (download, upload, count) = if enabled { totals(rows) } else { (0, 0, 0) };
        let next = (download, upload, count, locale, enabled);
        self.last.replace(next) != Some(next)
    }
}
