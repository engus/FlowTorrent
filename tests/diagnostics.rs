use flowtorrent::{
    diagnostics::BoundedLog,
    domain::*,
    error::AppError,
    ui::{i18n::Locale, models::to_row},
};
use std::{
    io::{Error, ErrorKind, Write},
    path::PathBuf,
};
use tracing_subscriber::fmt::MakeWriter;
#[test]
fn io_causes_keep_actionable_codes_even_inside_engine_context() {
    for (kind, expected) in [
        (ErrorKind::StorageFull, ErrorCode::DiskFull),
        (ErrorKind::PermissionDenied, ErrorCode::PermissionDenied),
        (ErrorKind::NotFound, ErrorCode::MissingVolume),
    ] {
        assert_eq!(
            AppError::Io(Error::new(kind, "fixture")).user_error().code,
            expected
        );
        let nested =
            anyhow::Error::new(Error::new(kind, "fixture")).context("engine storage failed");
        assert_eq!(AppError::Engine(nested).user_error().code, expected);
    }
}
#[test]
fn only_recoverable_row_errors_offer_retry() {
    let mut row = TorrentSnapshot {
        id: TorrentId([1; 20]),
        name: "file.bin".into(),
        selected_bytes: 1,
        checked_downloaded_bytes: 0,
        download_bps: 0,
        upload_bps: 0,
        state: TransferState::Error,
        error: None,
        destination: PathBuf::new(),
        desired_state: DesiredState::Active,
    };
    for code in [
        ErrorCode::InvalidMetadata,
        ErrorCode::UnsupportedFormat,
        ErrorCode::CorruptState,
    ] {
        row.error = Some(UserError { code, detail: None });
        assert!(!to_row(&row, Locale::En).can_retry);
    }
    for code in [
        ErrorCode::DiskFull,
        ErrorCode::PermissionDenied,
        ErrorCode::MissingVolume,
        ErrorCode::Engine,
    ] {
        row.error = Some(UserError { code, detail: None });
        assert!(to_row(&row, Locale::Ru).can_retry);
    }
}
#[test]
fn local_logs_bound_each_entry_and_total_disk_usage() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let log = BoundedLog::open(root.path().to_owned())?;
    for _ in 0..400 {
        let mut writer = log.make_writer();
        writer.write_all(&vec![b'x'; 20000])?;
    }
    let files: Vec<_> = std::fs::read_dir(root.path())?.collect::<Result<_, _>>()?;
    assert_eq!(files.len(), 2);
    let lengths: Vec<_> = files
        .iter()
        .map(|file| file.metadata().unwrap().len())
        .collect();
    assert!(lengths.iter().all(|size| *size <= 1024 * 1024));
    assert!(lengths.iter().sum::<u64>() <= 2 * 1024 * 1024);
    let bytes = std::fs::read(root.path().join("flowtorrent.log"))?;
    assert!(bytes.windows(13).any(|part| part == b"[truncated]\n\n"));
    Ok(())
}
