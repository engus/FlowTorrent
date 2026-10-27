#[path = "support/lifecycle.rs"]
mod lifecycle;
#[allow(dead_code)]
mod support;
use flowtorrent::{
    app::{
        commands::AppCommand,
        storage::{SessionRepository, SessionStore},
    },
    domain::*,
};
use std::sync::Arc;

#[tokio::test]
async fn restart_restores_paths_selection_and_pause_without_original_torrent() -> anyhow::Result<()>
{
    let root = support::tempdir()?;
    let store = Arc::new(SessionStore::new(lifecycle::paths(root.path())));
    let mut handle = lifecycle::start(root.path(), store.clone()).await?;
    let base = root.path().join("chosen");
    std::fs::create_dir(&base)?;
    let source = root.path().join("input.torrent");
    let metadata = support::multi_torrent_bytes(&vec![1; 32768], &vec![2; 32768]);
    let id = lifecycle::add(
        &mut handle,
        &source,
        &base,
        &metadata,
        Some(vec![FileId(1)]),
    )
    .await?;
    handle.commands.send(AppCommand::Pause(id)).await?;
    lifecycle::until(&mut handle, |s| {
        s.torrents[0].state == TransferState::Paused
    })
    .await?;
    lifecycle::stop(handle).await?;
    std::fs::remove_file(source)?;
    let mut saved = store.load()?;
    assert_eq!(saved.torrents[0].base_dir, base);
    assert_eq!(saved.torrents[0].selected_files, vec![FileId(1)]);
    assert_eq!(saved.torrents[0].desired_state, DesiredState::Paused);
    saved.torrents[0].completion_notified = true;
    store.save(&saved)?;
    let mut handle = lifecycle::start(root.path(), store.clone()).await?;
    let restored = lifecycle::until(&mut handle, |s| {
        s.torrents
            .first()
            .is_some_and(|row| row.state == TransferState::Paused)
    })
    .await?;
    assert_eq!(restored.torrents[0].selected_bytes, 32768);
    assert!(store.load()?.torrents[0].completion_notified);
    assert!(base.join("bundle/second.bin").exists());
    lifecycle::stop(handle).await
}

#[tokio::test]
async fn confirmed_remove_is_not_resurrected_from_leftover_metainfo() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let store = Arc::new(SessionStore::new(lifecycle::paths(root.path())));
    let mut handle = lifecycle::start(root.path(), store.clone()).await?;
    let id = lifecycle::add(
        &mut handle,
        &root.path().join("input.torrent"),
        &root.path().join("downloads"),
        &support::torrent_bytes(&support::payload(), false),
        None,
    )
    .await?;
    let metadata = store.load()?.torrents[0].metadata_path.clone();
    handle.commands.send(AppCommand::Forget(id)).await?;
    lifecycle::until(&mut handle, |s| s.torrents.is_empty()).await?;
    lifecycle::stop(handle).await?;
    assert!(!metadata.exists());
    // A stale cache from an earlier build still must not resurrect a removed row.
    std::fs::write(
        &metadata,
        support::torrent_bytes(&support::payload(), false),
    )?;
    assert!(root.path().join("downloads/fixture.bin").exists());
    let mut handle = lifecycle::start(root.path(), store.clone()).await?;
    handle.commands.send(AppCommand::SetVisible(false)).await?;
    lifecycle::until(&mut handle, |s| !s.visible).await?;
    assert!(handle.snapshots.borrow().torrents.is_empty());
    assert!(store.load()?.torrents.is_empty());
    lifecycle::stop(handle).await
}

#[test]
fn corrupt_and_future_manifest_are_preserved() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.data_dir)?;
    let store = SessionStore::new(paths.clone());
    for bytes in [
        b"{broken".as_slice(),
        br#"{"schema_version":99,"next_position":0,"torrents":[]}"#,
    ] {
        std::fs::write(&paths.manifest_file, bytes)?;
        assert!(store.load().is_err());
        assert!(store.save(&Manifest::default()).is_err());
        assert_eq!(std::fs::read(&paths.manifest_file)?, bytes);
    }
    Ok(())
}

#[test]
fn invalid_ownership_records_cannot_replace_a_valid_manifest() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let store = SessionStore::new(paths.clone());
    assert!(store.load()?.torrents.is_empty());
    assert!(!paths.manifest_file.exists());
    let id = TorrentId([7; 20]);
    let record = TorrentRecord {
        id,
        name: "fixture.bin".into(),
        position: 0,
        metadata_path: paths.metadata_dir.join(format!("{id}.torrent")),
        base_dir: paths.download_dir.clone(),
        selected_files: vec![FileId(0)],
        desired_state: DesiredState::Active,
        completion_notified: false,
    };
    let valid = Manifest {
        schema_version: 1,
        next_position: 1,
        torrents: vec![record.clone()],
    };
    store.save(&valid)?;
    let previous = std::fs::read(&paths.manifest_file)?;
    let mut invalids = vec![];
    let mut next = valid.clone();
    next.torrents.push(record.clone());
    invalids.push(next);
    let mut next = valid.clone();
    next.torrents[0].metadata_path = root.path().join("outside.torrent");
    invalids.push(next);
    let mut next = valid.clone();
    next.torrents[0].base_dir = "relative".into();
    invalids.push(next);
    let mut next = valid.clone();
    next.torrents[0].selected_files.clear();
    invalids.push(next);
    let mut next = valid.clone();
    next.torrents[0].selected_files.push(FileId(0));
    invalids.push(next);
    let mut next = valid.clone();
    next.next_position = 0;
    invalids.push(next);
    for invalid in invalids {
        assert!(store.save(&invalid).is_err());
        assert_eq!(std::fs::read(&paths.manifest_file)?, previous);
    }
    assert_eq!(store.load()?, valid);
    Ok(())
}

#[test]
fn missing_primary_with_backup_is_not_treated_as_an_empty_session() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let store = SessionStore::new(paths.clone());
    store.save(&Manifest::default())?;
    store.save(&Manifest::default())?;
    let backup = flowtorrent::persistence::backup_path(&paths.manifest_file)?;
    let previous = std::fs::read(&backup)?;
    std::fs::remove_file(&paths.manifest_file)?;
    assert!(store.load().is_err());
    assert!(store.save(&Manifest::default()).is_err());
    assert!(!paths.manifest_file.exists());
    assert_eq!(std::fs::read(backup)?, previous);
    Ok(())
}

#[tokio::test]
async fn missing_volume_stays_an_error_of_its_record_while_other_jobs_restore() -> anyhow::Result<()>
{
    let root = support::tempdir()?;
    let store = Arc::new(SessionStore::new(lifecycle::paths(root.path())));
    let mut handle = lifecycle::start(root.path(), store.clone()).await?;
    let mut ids = vec![];
    for index in 0..2 {
        let base = root.path().join(format!("volume{index}"));
        std::fs::create_dir(&base)?;
        ids.push(
            lifecycle::add(
                &mut handle,
                &root.path().join(format!("{index}.torrent")),
                &base,
                &support::torrent_bytes(&support::payload(), index == 1),
                None,
            )
            .await?,
        );
    }
    lifecycle::stop(handle).await?;
    std::fs::rename(
        root.path().join("volume0"),
        root.path().join("disconnected"),
    )?;
    let mut handle = lifecycle::start(root.path(), store.clone()).await?;
    let state = lifecycle::until(&mut handle, |s| {
        s.torrents.len() == 2
            && s.torrents
                .iter()
                .any(|r| r.id == ids[1] && r.state != TransferState::Error)
    })
    .await?;
    assert_eq!(
        state
            .torrents
            .iter()
            .find(|r| r.id == ids[0])
            .unwrap()
            .state,
        TransferState::Error
    );
    assert!(!root.path().join("volume0").exists());
    assert!(
        std::fs::read_dir(root.path().join("downloads"))?
            .next()
            .is_none()
    );
    std::fs::rename(
        root.path().join("disconnected"),
        root.path().join("volume0"),
    )?;
    handle.commands.send(AppCommand::Retry(ids[0])).await?;
    lifecycle::until(&mut handle, |s| {
        s.torrents.iter().all(|r| r.state != TransferState::Error)
    })
    .await?;
    lifecycle::stop(handle).await
}

#[tokio::test]
async fn missing_owned_payload_can_be_recreated_without_overwriting_other_files()
-> anyhow::Result<()> {
    let root = support::tempdir()?;
    let store = Arc::new(SessionStore::new(lifecycle::paths(root.path())));
    let mut handle = lifecycle::start(root.path(), store.clone()).await?;
    lifecycle::add(
        &mut handle,
        &root.path().join("input.torrent"),
        &root.path().join("downloads"),
        &support::torrent_bytes(&support::payload(), false),
        None,
    )
    .await?;
    lifecycle::stop(handle).await?;
    std::fs::remove_file(root.path().join("downloads/fixture.bin"))?;
    std::fs::write(root.path().join("downloads/mine.txt"), b"mine")?;
    let mut handle = lifecycle::start(root.path(), store.clone()).await?;
    lifecycle::until(&mut handle, |s| {
        s.torrents
            .first()
            .is_some_and(|r| r.state != TransferState::Error)
    })
    .await?;
    assert!(root.path().join("downloads/fixture.bin").exists());
    assert_eq!(
        std::fs::read(root.path().join("downloads/mine.txt"))?,
        b"mine"
    );
    lifecycle::stop(handle).await
}

#[tokio::test]
async fn mismatched_saved_metainfo_never_writes_payload() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let store = Arc::new(SessionStore::new(lifecycle::paths(root.path())));
    let mut handle = lifecycle::start(root.path(), store.clone()).await?;
    let id = lifecycle::add(
        &mut handle,
        &root.path().join("input.torrent"),
        &root.path().join("downloads"),
        &support::torrent_bytes(&support::payload(), false),
        None,
    )
    .await?;
    lifecycle::stop(handle).await?;
    let record = store.load()?.torrents.remove(0);
    std::fs::write(
        record.metadata_path,
        support::torrent_bytes(&support::payload(), true),
    )?;
    std::fs::write(root.path().join("downloads/fixture.bin"), b"unchanged")?;
    let mut handle = lifecycle::start(root.path(), store.clone()).await?;
    let state = lifecycle::until(&mut handle, |s| {
        s.torrents
            .first()
            .is_some_and(|r| r.state == TransferState::Error)
    })
    .await?;
    assert_eq!(state.torrents[0].id, id);
    assert_eq!(
        std::fs::read(root.path().join("downloads/fixture.bin"))?,
        b"unchanged"
    );
    lifecycle::stop(handle).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restart_reuses_verified_parts_then_finishes_the_known_payload() -> anyhow::Result<()> {
    use anyhow::Context;
    use flowtorrent::{
        app::AppController, engine::librqbit::LibrqbitEngine, settings::SettingsStore,
    };
    let root = support::tempdir()?;
    let source = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let store = Arc::new(SessionStore::new(paths.clone()));
    let payload = support::payload();
    let metadata = support::torrent_bytes(&payload, false);
    let (seeder, peer) = support::seed(source.path(), &metadata, &payload).await?;
    let mut handle = lifecycle::start(root.path(), store.clone()).await?;
    let id = lifecycle::add(
        &mut handle,
        &root.path().join("input.torrent"),
        &paths.download_dir,
        &metadata,
        None,
    )
    .await?;
    handle.commands.send(AppCommand::Pause(id)).await?;
    lifecycle::until(&mut handle, |s| {
        s.torrents[0].state == TransferState::Paused
    })
    .await
    .context("pause before writing the partial fixture")?;
    lifecycle::stop(handle).await?;
    let file = paths.download_dir.join("fixture.bin");
    std::fs::write(&file, &payload[..32768])?;
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
        paths,
    );
    let checked = lifecycle::until(&mut handle, |s| {
        s.torrents
            .first()
            .is_some_and(|r| r.state == TransferState::Paused)
    })
    .await
    .context("recheck paused partial payload")?;
    assert_eq!(checked.torrents[0].checked_downloaded_bytes, 32768);
    assert_eq!(&std::fs::read(&file)?[..32768], &payload[..32768]);
    handle.commands.send(AppCommand::Resume(id)).await?;
    lifecycle::until(&mut handle, |s| {
        s.torrents[0].state == TransferState::Seeding
    })
    .await
    .context("finish remaining pieces after resume")?;
    assert_eq!(std::fs::read(file)?, payload);
    lifecycle::stop(handle).await?;
    seeder.stop().await;
    Ok(())
}

#[tokio::test]
async fn corrupt_manifest_error_remains_visible_after_hiding_and_showing() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.data_dir)?;
    std::fs::write(&paths.manifest_file, b"{broken")?;
    let mut handle =
        lifecycle::start(root.path(), Arc::new(SessionStore::new(paths.clone()))).await?;
    let initial = lifecycle::until(&mut handle, |s| s.error.is_some()).await?;
    assert_eq!(
        initial.error.as_ref().unwrap().code,
        ErrorCode::CorruptState
    );
    for visible in [false, true] {
        handle
            .commands
            .send(AppCommand::SetVisible(visible))
            .await?;
        let state = lifecycle::until(&mut handle, |s| s.visible == visible).await?;
        assert_eq!(state.error, initial.error);
    }
    assert_eq!(std::fs::read(paths.manifest_file)?, b"{broken");
    lifecycle::stop(handle).await
}
