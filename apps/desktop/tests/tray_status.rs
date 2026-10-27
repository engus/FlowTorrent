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
        checking: None,
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
    assert_eq!(status.eta, "—");
    assert!(status.tooltip.contains("FlowTorrent"));
    let empty = TrayStatus::from_rows(&[], Locale::Ru);
    assert_eq!(
        (empty.download_bps, empty.upload_bps, empty.downloading),
        (0, 0, 0)
    );
    assert!(empty.eta == "—");
    assert!(empty.download.contains("0 Б/с"));
}
#[test]
fn compact_summary_is_single_line_localized_and_keeps_aggregate_semantics() {
    let rows = [
        row(1, TransferState::Downloading, 1_000_000, 50_000),
        row(2, TransferState::Seeding, 0, 200_000),
        row(3, TransferState::Paused, 0, 0),
    ];
    let en = TrayStatus::from_rows(&rows, Locale::En);
    assert_eq!(en.summary, "less 1 min · ↓ 1 MB/s · ↑ 250 KB/s");
    let ru = TrayStatus::from_rows(&rows, Locale::Ru);
    assert_eq!(ru.summary, "меньше 1 мин · ↓ 1 МБ/с · ↑ 250 КБ/с");
    for locale in Locale::ALL {
        let status = TrayStatus::from_rows(&rows, locale);
        assert!(!status.summary.contains('\n'));
        assert!(status.summary.contains(&status.eta));
        assert_eq!(status.downloading, 1);
    }
    assert_eq!(
        TrayStatus::from_rows(&[], Locale::En).summary,
        "— · ↓ 0 B/s · ↑ 0 B/s"
    );
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
    assert!(refresh.changed(&[], Locale::En, true));
    assert!(!refresh.changed(&[], Locale::En, true));
    let mut rows = [row(1, TransferState::Downloading, 20_000, 1000)];
    assert!(refresh.changed(&rows, Locale::En, true));
    assert!(!refresh.changed(&rows, Locale::En, true));
    rows[0].download_bps = 21_000;
    assert!(refresh.changed(&rows, Locale::En, true));
    rows[0].upload_bps = 2000;
    assert!(refresh.changed(&rows, Locale::En, true));
    rows[0].state = TransferState::Paused;
    assert!(refresh.changed(&rows, Locale::En, true));
    assert!(refresh.changed(&rows, Locale::Ja, true));
    assert!(!refresh.changed(&rows, Locale::Ja, true));
    assert!(refresh.changed(&[], Locale::Ja, true));
    assert!(!refresh.changed(&[], Locale::Ja, true));
}

#[test]
fn disabled_tray_sleeps_through_rate_changes_but_wakes_for_preference_changes() {
    use flowtorrent::ui::tray_status::TrayRefresh;
    let mut refresh = TrayRefresh::default();
    let mut rows = [row(1, TransferState::Downloading, 20_000, 0)];
    assert!(refresh.changed(&rows, Locale::En, true));
    assert!(refresh.changed(&rows, Locale::En, false));
    rows[0].download_bps = 90_000;
    assert!(!refresh.changed(&rows, Locale::En, false));
    assert!(refresh.changed(&rows, Locale::En, true));
    assert!(!refresh.changed(&rows, Locale::En, true));
}

#[test]
fn eta_is_the_longest_active_transfer_and_ignores_paused_and_complete_rows() {
    let mut first = row(1, TransferState::Downloading, 100, 0);
    first.selected_bytes = 6_000;
    let mut last = row(2, TransferState::Downloading, 500, 0);
    last.selected_bytes = 100_000;
    let mut paused = row(3, TransferState::Downloading, 1, 0);
    paused.desired_state = DesiredState::Paused;
    paused.selected_bytes = u64::MAX;
    let seed = row(4, TransferState::Seeding, 0, 100);
    let mut rows = [first, last, paused, seed];
    assert_eq!(TrayStatus::from_rows(&rows, Locale::En).eta, "00:04");
    use flowtorrent::ui::tray_status::TrayRefresh;
    let mut refresh = TrayRefresh::default();
    assert!(refresh.changed(&rows, Locale::En, true));
    assert!(!refresh.changed(&rows, Locale::En, true));
    rows[0].checked_downloaded_bytes = 5_900;
    rows[1].checked_downloaded_bytes = 99_000;
    assert!(
        refresh.changed(&rows, Locale::En, true),
        "same-rate progress must refresh ETA"
    );
    assert_eq!(TrayStatus::from_rows(&rows, Locale::En).eta, "less 1 min");
    assert!(!refresh.changed(&rows, Locale::En, true));
    assert!(refresh.changed(&rows, Locale::En, false));
    rows[1].checked_downloaded_bytes += 500;
    assert!(!refresh.changed(&rows, Locale::En, false));
}

#[test]
fn any_active_unknown_transfer_makes_overall_eta_unknown() {
    for state in [
        TransferState::Checking,
        TransferState::Connecting,
        TransferState::WaitingForSources,
        TransferState::Error,
    ] {
        let rows = [
            row(1, TransferState::Downloading, 100, 0),
            row(2, state, 100, 0),
        ];
        assert_eq!(
            TrayStatus::from_rows(&rows, Locale::En).eta,
            "—",
            "{state:?}"
        );
    }
    let mut unknown = row(1, TransferState::Downloading, 0, 0);
    assert_eq!(
        TrayStatus::from_rows(&[unknown.clone()], Locale::Ru).eta,
        "—"
    );
    unknown.selected_bytes = 0;
    unknown.download_bps = 100;
    assert_eq!(TrayStatus::from_rows(&[unknown], Locale::En).eta, "—");
    let mut complete = row(1, TransferState::Downloading, 0, 0);
    complete.checked_downloaded_bytes = complete.selected_bytes;
    assert_eq!(TrayStatus::from_rows(&[complete], Locale::En).eta, "—");
}

#[test]
fn eta_rounds_up_without_byte_or_time_overflow() {
    let mut r = row(1, TransferState::Downloading, 99, 0);
    assert_eq!(
        TrayStatus::from_rows(&[r.clone()], Locale::En).eta,
        "less 1 min"
    );
    r.selected_bytes = u64::MAX;
    r.download_bps = 1;
    let seconds = u64::MAX;
    assert_eq!(
        TrayStatus::from_rows(&[r], Locale::En).eta,
        format!(
            "{:02}:{:02}",
            seconds.div_ceil(60) / 60,
            seconds.div_ceil(60) % 60
        )
    );
}

#[test]
fn eta_uses_localized_minutes_and_does_not_wake_for_seconds() {
    use flowtorrent::ui::tray_status::TrayRefresh;
    let mut r = row(1, TransferState::Downloading, 1, 0);
    r.selected_bytes = 59;
    assert_eq!(
        TrayStatus::from_rows(&[r.clone()], Locale::En).eta,
        "less 1 min"
    );
    assert_eq!(
        TrayStatus::from_rows(&[r.clone()], Locale::Ru).eta,
        "меньше 1 мин"
    );
    r.selected_bytes = 60;
    assert_eq!(TrayStatus::from_rows(&[r.clone()], Locale::En).eta, "00:01");
    r.selected_bytes = 61;
    assert_eq!(TrayStatus::from_rows(&[r.clone()], Locale::En).eta, "00:02");
    let mut refresh = TrayRefresh::default();
    assert!(refresh.changed(&[r.clone()], Locale::En, true));
    r.selected_bytes = 119;
    assert!(
        !refresh.changed(&[r.clone()], Locale::En, true),
        "same displayed minute must not wake UI"
    );
    r.selected_bytes = 3_600;
    assert_eq!(TrayStatus::from_rows(&[r.clone()], Locale::En).eta, "01:00");
    r.selected_bytes = 300;
    assert_eq!(TrayStatus::from_rows(&[r.clone()], Locale::En).eta, "00:05");
    r.selected_bytes = 4_800;
    assert_eq!(TrayStatus::from_rows(&[r.clone()], Locale::En).eta, "01:20");
}
