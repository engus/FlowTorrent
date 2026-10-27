use super::{
    i18n::{Locale, format_bytes},
    view_state::{complete, eta_seconds},
};
use crate::domain::{DesiredState, TorrentSnapshot, TransferState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayStatus {
    pub download_bps: u64,
    pub upload_bps: u64,
    pub downloading: usize,
    pub download: String,
    pub upload: String,
    pub eta: String,
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
        let eta = display_eta(eta_minutes(rows), locale);
        let summary = format!(
            "{eta} · ↓ {}{suffix} · ↑ {}{suffix}",
            format_bytes(download_bps, locale),
            format_bytes(upload_bps, locale),
        );
        let remaining = locale.text("Оставшееся время", "Time remaining");
        let tooltip = format!("FlowTorrent\n{download}\n{upload}\n{remaining}: {eta}");
        Self {
            download_bps,
            upload_bps,
            downloading,
            download,
            upload,
            eta,
            tooltip,
            summary,
        }
    }
}

fn aggregate_eta(rows: &[TorrentSnapshot]) -> Option<u64> {
    let mut longest = None;
    for row in rows.iter().filter(|row| {
        row.desired_state == DesiredState::Active
            && row.state != TransferState::Paused
            && !complete(row)
    }) {
        if row.state != TransferState::Downloading || row.selected_bytes == 0 {
            return None;
        }
        let seconds = eta_seconds(
            row.selected_bytes,
            row.checked_downloaded_bytes,
            row.download_bps,
        )?;
        longest = Some(longest.unwrap_or(0).max(seconds));
    }
    longest
}

fn eta_minutes(rows: &[TorrentSnapshot]) -> Option<u64> {
    aggregate_eta(rows).map(|seconds| {
        if seconds < 60 {
            0
        } else {
            seconds.div_ceil(60)
        }
    })
}
fn display_eta(minutes: Option<u64>, locale: Locale) -> String {
    let Some(minutes) = minutes else {
        return "—".to_owned();
    };
    if minutes == 0 {
        locale.text("меньше 1 мин", "less 1 min").to_owned()
    } else {
        format!("{:02}:{:02}", minutes / 60, minutes % 60)
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
    last: Option<(u64, u64, Option<u64>, Locale, bool)>,
}
impl TrayRefresh {
    pub fn changed(&mut self, rows: &[TorrentSnapshot], locale: Locale, enabled: bool) -> bool {
        let (download, upload, _) = if enabled { totals(rows) } else { (0, 0, 0) };
        let eta = if enabled { eta_minutes(rows) } else { None };
        let next = (download, upload, eta, locale, enabled);
        self.last.replace(next) != Some(next)
    }
}
