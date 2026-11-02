#[path = "support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod support;
use flowtorrent::{
    app::storage::{SessionRepository, SessionStore},
    domain::*,
    platform::notifications::completion_event,
};

fn record(root: &std::path::Path) -> TorrentRecord {
    let paths = lifecycle::paths(root);
    let id = TorrentId([3; 20]);
    TorrentRecord {
        priority: Default::default(),
        id,
        name: "bundle".into(),
        position: 0,
        metadata_path: paths.metadata_dir.join(format!("{id}.torrent")),
        base_dir: paths.download_dir,
        selected_files: vec![],
        desired_state: DesiredState::Active,
        completion_notified: false,
        file_priorities: vec![],
    }
}
#[test]
fn empty_selection_is_versioned_and_future_or_corrupt_state_is_preserved() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let store = SessionStore::new(paths.clone());
    let mut manifest = Manifest {
        schema_version: 2,
        next_position: 1,
        torrents: vec![record(root.path())],
    };
    store.save(&manifest)?;
    assert_eq!(store.load()?, manifest);
    manifest.schema_version = 1;
    assert!(store.save(&manifest).is_err());
    for original in [br#"{"schema_version":3}"#.to_vec(), b"{ broken".to_vec()] {
        std::fs::write(&paths.manifest_file, &original)?;
        assert!(store.load().is_err());
        assert!(store.save(&Manifest::default()).is_err());
        assert_eq!(std::fs::read(&paths.manifest_file)?, original);
    }
    Ok(())
}
#[test]
fn no_selected_bytes_never_notifies_or_marks_completion() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let mut record = record(root.path());
    let snapshot = TorrentSnapshot {
        priority: Default::default(),
        id: record.id,
        name: record.name.clone(),
        selected_bytes: 0,
        checked_downloaded_bytes: 0,
        checking: None,
        download_bps: 0,
        upload_bps: 0,
        session_uploaded_bytes: 0,
        state: TransferState::Seeding,
        error: None,
        destination: record.base_dir.clone(),
        desired_state: DesiredState::Active,
    };
    assert!(completion_event(&mut record, &snapshot, true).is_none());
    assert!(!record.completion_notified);
    Ok(())
}
