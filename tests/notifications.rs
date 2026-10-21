#[path = "support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod support;
use flowtorrent::{
    app::{
        AppController, AppEvent, AppHandle,
        commands::AppCommand,
        storage::{SessionRepository, SessionStore},
    },
    domain::*,
    engine::librqbit::LibrqbitEngine,
    platform::notifications::{NotificationSink, completion_event, deliver_notice},
    settings::{Settings, SettingsStore},
};
use std::{
    num::NonZeroU32,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn record() -> TorrentRecord {
    TorrentRecord {
        id: TorrentId([1; 20]),
        name: "имя & <file>.bin".into(),
        position: 0,
        metadata_path: PathBuf::from("metadata"),
        base_dir: PathBuf::from("downloads"),
        selected_files: vec![FileId(1)],
        desired_state: DesiredState::Active,
        completion_notified: false,
    }
}
fn snapshot(record: &TorrentRecord, state: TransferState) -> TorrentSnapshot {
    TorrentSnapshot {
        id: record.id,
        name: record.name.clone(),
        selected_bytes: 20,
        checked_downloaded_bytes: 20,
        state,
        error: None,
        download_bps: 0,
        upload_bps: 0,
        destination: record.base_dir.clone(),
        desired_state: record.desired_state,
    }
}
#[test]
fn selected_files_completion_is_enough_and_waiting_is_not_an_error() {
    let mut record = record();
    let mut row = snapshot(&record, TransferState::WaitingForSources);
    assert!(completion_event(&mut record, &row, true).is_none());
    assert!(!record.completion_notified);
    row.state = TransferState::Seeding;
    let notice = completion_event(&mut record, &row, true).unwrap();
    assert_eq!(notice.id, record.id);
    assert_eq!(notice.name, record.name);
    assert!(record.completion_notified);
    row.state = TransferState::Completed;
    assert!(completion_event(&mut record, &row, true).is_none());
}
#[test]
fn disabled_and_denied_notifications_do_not_break_downloads_or_create_old_notices() {
    let mut record = record();
    let row = snapshot(&record, TransferState::Seeding);
    assert!(completion_event(&mut record, &row, false).is_none());
    assert!(record.completion_notified);
    assert!(completion_event(&mut record, &row, true).is_none());
    struct Denied(AtomicUsize);
    impl NotificationSink for Denied {
        fn completed(&self, _: TorrentId, _: &str) -> flowtorrent::error::AppResult<()> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "OS denied notifications",
            )
            .into())
        }
    }
    record.completion_notified = false;
    let notice = completion_event(&mut record, &row, true).unwrap();
    let sink = Denied(AtomicUsize::new(0));
    deliver_notice(&sink, notice);
    assert_eq!(sink.0.load(Ordering::SeqCst), 1);
    assert_eq!(row.state, TransferState::Seeding);
    assert!(completion_event(&mut record, &row, true).is_none());
}
async fn notices_for(handle: &mut AppHandle, duration: Duration) -> Vec<TorrentId> {
    let mut notices = Vec::new();
    let _ = tokio::time::timeout(duration, async {
        while let Some(event) = handle.events.recv().await {
            if let AppEvent::Completion(notice) = event {
                notices.push(notice.id);
            }
        }
    })
    .await;
    notices
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_completion_notice_is_durable_before_delivery_and_stays_once_across_restart()
-> anyhow::Result<()> {
    let root = support::tempdir()?;
    let source = support::tempdir()?;
    let payload = support::payload()[..65536].to_vec();
    let metadata = support::torrent_bytes(&payload, false);
    let (seeder, peer) = support::seed(source.path(), &metadata, &payload).await?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.download_dir)?;
    let settings_store = Arc::new(SettingsStore::new(
        paths.config_file.clone(),
        paths.download_dir.clone(),
    ));
    let mut settings = Settings::defaults(paths.download_dir.clone());
    settings.limits.download_bps = NonZeroU32::new(20000);
    settings_store.save(&settings)?;
    let store = Arc::new(SessionStore::new(paths.clone()));
    let engine = LibrqbitEngine::with_options(
        paths.download_dir.clone(),
        support::local_options(),
        vec![peer],
    )
    .await?;
    let mut handle = AppController::spawn(
        Box::new(engine),
        store.clone(),
        settings_store,
        paths.clone(),
    );
    let id = lifecycle::add(
        &mut handle,
        &root.path().join("input.torrent"),
        &paths.download_dir,
        &metadata,
        None,
    )
    .await?;
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Some(AppEvent::Completion(notice)) = handle.events.recv().await {
                assert_eq!(notice.id, id);
                assert!(store.load().unwrap().torrents[0].completion_notified);
                break;
            }
        }
    })
    .await?;
    assert!(
        notices_for(&mut handle, Duration::from_millis(600))
            .await
            .is_empty()
    );
    lifecycle::stop(handle).await?;
    seeder.stop().await;
    std::fs::remove_file(root.path().join("input.torrent"))?;
    let mut restarted = lifecycle::start(root.path(), store.clone()).await?;
    lifecycle::until(&mut restarted, |s| {
        s.torrents
            .first()
            .is_some_and(|r| r.state == TransferState::Seeding)
    })
    .await?;
    assert!(store.load()?.torrents[0].completion_notified);
    assert!(
        notices_for(&mut restarted, Duration::from_millis(1100))
            .await
            .is_empty()
    );
    restarted.commands.send(AppCommand::Pause(id)).await?;
    lifecycle::until(&mut restarted, |s| {
        s.torrents[0].state == TransferState::Completed
    })
    .await?;
    lifecycle::stop(restarted).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_completion_save_keeps_seeding_without_notice_or_repeated_ui_errors()
-> anyhow::Result<()> {
    use std::sync::atomic::AtomicBool;
    struct CompletionStore {
        inner: SessionStore,
        full: AtomicBool,
        destination: PathBuf,
    }
    impl SessionRepository for CompletionStore {
        fn load(&self) -> flowtorrent::error::AppResult<Manifest> {
            self.inner.load()
        }
        fn save(&self, manifest: &Manifest) -> flowtorrent::error::AppResult<()> {
            if self.full.load(Ordering::SeqCst)
                && manifest
                    .torrents
                    .iter()
                    .any(|record| record.completion_notified)
            {
                return Err(flowtorrent::error::AppError::DiskFull(
                    self.destination.clone(),
                ));
            }
            self.inner.save(manifest)
        }
    }
    let root = support::tempdir()?;
    let source = support::tempdir()?;
    let payload = support::payload()[..16384].to_vec();
    let metadata = support::torrent_bytes(&payload, false);
    let (seeder, peer) = support::seed(source.path(), &metadata, &payload).await?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.download_dir)?;
    let store = Arc::new(CompletionStore {
        inner: SessionStore::new(paths.clone()),
        full: AtomicBool::new(true),
        destination: paths.download_dir.clone(),
    });
    let engine = LibrqbitEngine::with_options(
        paths.download_dir.clone(),
        support::local_options(),
        vec![peer],
    )
    .await?;
    let mut handle = AppController::spawn(
        Box::new(engine),
        store.clone(),
        Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        )),
        paths.clone(),
    );
    lifecycle::add(
        &mut handle,
        &root.path().join("input.torrent"),
        &paths.download_dir,
        &metadata,
        None,
    )
    .await?;
    lifecycle::until(&mut handle, |s| {
        s.torrents[0].state == TransferState::Seeding && s.error.is_some()
    })
    .await?;
    let mut errors = 0;
    let _ = tokio::time::timeout(Duration::from_millis(1700), async {
        while let Some(event) = handle.events.recv().await {
            match event {
                AppEvent::Completion(_) => panic!("notice before durable completion flag"),
                AppEvent::Error { .. } => errors += 1,
                _ => {}
            }
        }
    })
    .await;
    assert!(
        errors <= 1,
        "unchanged disk-full error repeated {errors} times"
    );
    assert_eq!(
        handle.snapshots.borrow().torrents[0].state,
        TransferState::Seeding
    );
    assert!(!store.load()?.torrents[0].completion_notified);
    store.full.store(false, Ordering::SeqCst);
    let notices = notices_for(&mut handle, Duration::from_millis(1100)).await;
    assert_eq!(notices.len(), 1);
    assert!(store.load()?.torrents[0].completion_notified);
    assert!(handle.snapshots.borrow().error.is_none());
    lifecycle::stop(handle).await?;
    seeder.stop().await;
    Ok(())
}
