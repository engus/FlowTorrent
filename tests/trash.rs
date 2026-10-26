use flowtorrent::{
    app::{targets::prepare_add, trash::TrashStage},
    domain::*,
};
use std::sync::Arc;
fn fixture() -> anyhow::Result<(
    tempfile::TempDir,
    Arc<flowtorrent::app::targets::PreparedAdd>,
    TorrentId,
)> {
    let root = tempfile::tempdir()?;
    let base = root.path().canonicalize()?;
    let preview = TorrentPreview {
        id: TorrentId([8; 20]),
        name: "bundle".into(),
        multi_file: true,
        metadata: vec![],
        files: vec![TorrentFile {
            id: FileId(0),
            relative_path: "inner/file.bin".into(),
            length: 4,
            is_padding: false,
        }],
    };
    let prepared = Arc::new(prepare_add(
        &preview,
        &AddRequest::new(preview.id, base, vec![FileId(0)]),
    )?);
    prepared.claim_files()?;
    Ok((root, prepared, preview.id))
}
#[test]
fn trash_stages_owned_files_only_and_preserves_recovery_on_backend_failure() -> anyhow::Result<()> {
    let (root, prepared, id) = fixture()?;
    let base = root.path().canonicalize()?;
    std::fs::write(base.join("bundle/unrelated.txt"), "keep")?;
    let journal = base.join("recovery.json");
    let stage = prepared.stage_for_trash(id, &[FileId(0)], &journal)?;
    assert!(!base.join("bundle/inner/file.bin").exists());
    assert_eq!(
        std::fs::read_to_string(base.join("bundle/unrelated.txt"))?,
        "keep"
    );
    assert!(
        stage
            .finish_with(|_| Err(std::io::Error::other("Trash unavailable").into()))
            .is_err()
    );
    assert!(journal.exists());
    let stage = TrashStage::load(&journal, id)?;
    let simulated_trash = base.join("simulated-trash");
    stage.finish_with(|path| Ok(std::fs::rename(path, &simulated_trash)?))?;
    assert!(
        journal.exists(),
        "receipt must survive until the registry save succeeds"
    );
    assert!(TrashStage::is_trashed(&journal, id)?);
    TrashStage::finish_registry_removal(&journal, id)?;
    assert!(!journal.exists());
    assert!(simulated_trash.join("bundle/inner/file.bin").is_file());
    assert!(base.join("bundle/unrelated.txt").exists());
    Ok(())
}
#[test]
fn trash_refuses_replaced_files_before_staging() -> anyhow::Result<()> {
    let (root, prepared, id) = fixture()?;
    let base = root.path().canonicalize()?;
    std::fs::rename(base.join("bundle/inner/file.bin"), base.join("original"))?;
    std::fs::write(base.join("bundle/inner/file.bin"), "foreign")?;
    assert!(
        prepared
            .stage_for_trash(id, &[FileId(0)], &base.join("recovery.json"))
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(base.join("bundle/inner/file.bin"))?,
        "foreign"
    );
    Ok(())
}
#[cfg(unix)]
#[test]
fn trash_rejects_hardlinks_and_symlinks() -> anyhow::Result<()> {
    let (root, prepared, id) = fixture()?;
    let base = root.path().canonicalize()?;
    std::fs::hard_link(base.join("bundle/inner/file.bin"), base.join("extra-link"))?;
    assert!(
        prepared
            .stage_for_trash(id, &[FileId(0)], &base.join("recovery.json"))
            .is_err()
    );
    std::fs::remove_file(base.join("extra-link"))?;
    std::fs::rename(base.join("bundle/inner/file.bin"), base.join("original"))?;
    std::os::unix::fs::symlink(base.join("original"), base.join("bundle/inner/file.bin"))?;
    assert!(
        prepared
            .stage_for_trash(id, &[FileId(0)], &base.join("recovery.json"))
            .is_err()
    );
    assert!(base.join("original").exists());
    Ok(())
}
#[test]
fn recovery_rejects_unexpected_files_and_wrong_torrent() -> anyhow::Result<()> {
    let (root, prepared, id) = fixture()?;
    let base = root.path().canonicalize()?;
    let journal = base.join("recovery.json");
    let stage = prepared.stage_for_trash(id, &[FileId(0)], &journal)?;
    std::fs::write(stage.path().join("foreign"), "keep")?;
    assert!(TrashStage::load(&journal, TorrentId([9; 20])).is_err());
    assert!(
        stage
            .finish_with(|_| panic!("must not reach OS Trash"))
            .is_err()
    );
    assert!(journal.exists());
    Ok(())
}
#[path = "support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod support;
#[tokio::test]
async fn interrupted_trash_blocks_restore_and_retry_without_recreating_payload()
-> anyhow::Result<()> {
    use flowtorrent::{
        app::{
            commands::AppCommand,
            storage::{SessionRepository, SessionStore},
        },
        domain::ErrorCode,
    };
    let root = tempfile::tempdir()?;
    let base = root.path().canonicalize()?;
    let paths = lifecycle::paths(&base);
    std::fs::create_dir_all(&paths.download_dir)?;
    let preview = TorrentPreview {
        id: TorrentId([6; 20]),
        name: "file.bin".into(),
        multi_file: false,
        metadata: vec![],
        files: vec![TorrentFile {
            id: FileId(0),
            relative_path: "file.bin".into(),
            length: 4,
            is_padding: false,
        }],
    };
    let prepared = Arc::new(prepare_add(
        &preview,
        &AddRequest::new(preview.id, paths.download_dir.clone(), vec![FileId(0)]),
    )?);
    prepared.claim_files()?;
    let journal = paths
        .data_dir
        .join("trash")
        .join(format!("{}.json", preview.id));
    let stage = prepared.stage_for_trash(preview.id, &[FileId(0)], &journal)?;
    stage.finish_with(|path| Ok(std::fs::rename(path, base.join("simulated-trash"))?))?;
    let store = Arc::new(SessionStore::new(paths.clone()));
    store.save(&Manifest {
        schema_version: 1,
        next_position: 1,
        torrents: vec![TorrentRecord {
            id: preview.id,
            name: preview.name,
            position: 0,
            metadata_path: paths.metadata_dir.join(format!("{}.torrent", preview.id)),
            base_dir: paths.download_dir.clone(),
            selected_files: vec![FileId(0)],
            desired_state: DesiredState::Active,
            completion_notified: false,
        }],
    })?;
    let mut handle = lifecycle::start(&base, store).await?;
    lifecycle::until(&mut handle, |s| {
        s.torrents
            .first()
            .and_then(|r| r.error.as_ref())
            .is_some_and(|e| e.code == ErrorCode::TrashRecovery)
    })
    .await?;
    assert!(!paths.download_dir.join("file.bin").exists());
    while handle.events.try_recv().is_ok() {}
    handle
        .commands
        .send(AppCommand::ConfirmAdd(AddRequest::new(
            preview.id,
            paths.download_dir.clone(),
            vec![FileId(0)],
        )))
        .await?;
    while let Some(event) = handle.events.recv().await {
        if matches!(
            event,
            flowtorrent::app::AppEvent::CommandFinished {
                operation: flowtorrent::app::commands::AppOperation::ConfirmAdd,
                ..
            }
        ) {
            break;
        }
    }
    assert!(
        journal.exists(),
        "a stale add must not erase a receipt while the registry still owns the torrent"
    );
    handle.commands.send(AppCommand::Retry(preview.id)).await?;
    handle
        .commands
        .send(AppCommand::SetAllActive(false))
        .await?;
    lifecycle::until(&mut handle, |s| s.error.is_some()).await?;
    assert!(!paths.download_dir.join("file.bin").exists());
    assert!(journal.exists());
    lifecycle::stop(handle).await
}
#[tokio::test]
async fn stopped_torrent_can_move_to_trash_with_a_fake_native_backend() -> anyhow::Result<()> {
    use flowtorrent::{
        app::{AppController, commands::AppCommand, storage::SessionStore},
        engine::librqbit::LibrqbitEngine,
        settings::SettingsStore,
    };
    let root = tempfile::tempdir()?;
    let base = root.path().canonicalize()?;
    let paths = lifecycle::paths(&base);
    std::fs::create_dir_all(&paths.download_dir)?;
    let engine =
        LibrqbitEngine::with_options(paths.download_dir.clone(), support::local_options(), vec![])
            .await?;
    let trash_bin = base.join("fake-bin");
    let saved_bin = trash_bin.clone();
    let mut handle = AppController::spawn_with_trash(
        Box::new(engine),
        Arc::new(SessionStore::new(paths.clone())),
        Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        )),
        paths.clone(),
        Arc::new(move |path| Ok(std::fs::rename(path, &trash_bin)?)),
    );
    let metadata = support::torrent_bytes(&support::payload(), false);
    let id = lifecycle::add(
        &mut handle,
        &base.join("input.torrent"),
        &paths.download_dir,
        &metadata,
        None,
    )
    .await?;
    handle.commands.send(AppCommand::Pause(id)).await?;
    lifecycle::until(&mut handle, |s| {
        s.torrents
            .first()
            .is_some_and(|r| r.state == TransferState::Paused)
    })
    .await?;
    handle.commands.send(AppCommand::Trash(id)).await?;
    let result =
        lifecycle::until(&mut handle, |s| s.torrents.is_empty() || s.error.is_some()).await?;
    assert!(
        result.torrents.is_empty(),
        "stopped torrent could not be trashed: {:?}",
        result.error
    );
    assert!(saved_bin.join("fixture.bin").exists());
    assert!(
        !paths
            .data_dir
            .join("trash")
            .join(format!("{id}.json"))
            .exists()
    );
    lifecycle::stop(handle).await
}
