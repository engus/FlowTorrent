use flowtorrent::{
    domain::*,
    ui::{i18n::Locale, tray_status::TrayStatus},
};
use std::path::PathBuf;
fn row(n: u8, state: TransferState, download: u64, upload: u64) -> TorrentSnapshot {
    TorrentSnapshot {
        id: TorrentId([n; 20]),
        name: format!("Item {n}"),
        selected_bytes: 100,
        checked_downloaded_bytes: 0,
        download_bps: download,
        upload_bps: upload,
        session_uploaded_bytes: 0,
        state,
        error: None,
        destination: PathBuf::new(),
        desired_state: DesiredState::Active,
    }
}
#[test]
fn tray_totals_are_registry_wide_and_count_actual_downloads() {
    let rows = [
        row(1, TransferState::Downloading, 1_000_000, 50_000),
        row(2, TransferState::Seeding, 0, 200_000),
        row(3, TransferState::Paused, 0, 0),
        row(4, TransferState::WaitingForSources, 0, 0),
        row(5, TransferState::Connecting, 0, 0),
        row(6, TransferState::Error, 0, 0),
    ];
    let status = TrayStatus::from_rows(&rows, Locale::En);
    assert_eq!(
        (status.download_bps, status.upload_bps, status.downloading),
        (1_000_000, 250_000, 1)
    );
    assert!(status.download.contains("1 MB/s"));
    assert!(status.upload.contains("250 KB/s"));
    assert!(status.active.contains("1"));
    assert!(status.tooltip.contains("FlowTorrent"));
    let empty = TrayStatus::from_rows(&[], Locale::Ru);
    assert_eq!(
        (empty.download_bps, empty.upload_bps, empty.downloading),
        (0, 0, 0)
    );
    assert!(empty.active.contains("Скачиваются"));
    assert!(empty.download.contains("0 Б/с"));
}
#[test]
fn aggregate_counter_overflow_saturates_instead_of_panicking() {
    let status = TrayStatus::from_rows(
        &[
            row(1, TransferState::Downloading, u64::MAX, u64::MAX),
            row(2, TransferState::Downloading, 20, 30),
        ],
        Locale::En,
    );
    assert_eq!(
        (status.download_bps, status.upload_bps, status.downloading),
        (u64::MAX, u64::MAX, 2)
    );
}

#[test]
fn hidden_tray_wakes_only_for_changed_rates_counts_or_language_including_zero() {
    use flowtorrent::ui::tray_status::TrayRefresh;
    let mut refresh = TrayRefresh::default();
    assert!(refresh.changed(&[], Locale::En));
    assert!(!refresh.changed(&[], Locale::En));
    let mut rows = [row(1, TransferState::Downloading, 20_000, 1000)];
    assert!(refresh.changed(&rows, Locale::En));
    assert!(!refresh.changed(&rows, Locale::En));
    rows[0].download_bps = 21_000;
    assert!(refresh.changed(&rows, Locale::En));
    rows[0].upload_bps = 2000;
    assert!(refresh.changed(&rows, Locale::En));
    rows[0].state = TransferState::Paused;
    assert!(refresh.changed(&rows, Locale::En));
    assert!(refresh.changed(&rows, Locale::Ja));
    assert!(!refresh.changed(&rows, Locale::Ja));
    assert!(refresh.changed(&[], Locale::Ja));
    assert!(!refresh.changed(&[], Locale::Ja));
}
