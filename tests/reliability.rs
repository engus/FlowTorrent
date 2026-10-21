#[path = "support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod support;
#[cfg(windows)]
use flowtorrent::domain::*;
use flowtorrent::{
    app::{
        AppEvent,
        commands::{AppCommand, AppOperation},
        storage::SessionStore,
    },
    engine::{TorrentEngine, librqbit::LibrqbitEngine},
};
use std::{sync::Arc, time::Duration};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejected_commands_preserve_the_original_startup_failure() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.data_dir)?;
    std::fs::write(&paths.manifest_file, b"{broken")?;
    let mut handle =
        lifecycle::start(root.path(), Arc::new(SessionStore::new(paths.clone()))).await?;
    let initial = lifecycle::until(&mut handle, |s| s.error.is_some()).await?;
    handle
        .commands
        .send(AppCommand::OpenTorrent(root.path().join("input.torrent")))
        .await?;
    tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(event) = handle.events.recv().await {
            if matches!(
                event,
                AppEvent::CommandFinished {
                    operation: AppOperation::OpenTorrent,
                    success: false
                }
            ) {
                break;
            }
        }
    })
    .await?;
    let actual = handle.snapshots.borrow().error.clone();
    lifecycle::stop(handle).await?;
    assert_eq!(actual, initial.error);
    assert_eq!(std::fs::read(paths.manifest_file)?, b"{broken");
    Ok(())
}

#[test]
fn deep_unknown_metadata_is_rejected_before_the_recursive_parser() -> anyhow::Result<()> {
    // A large fixture-only stack lets the unfixed parser handle the first 140
    // levels safely and produce an ordinary failing assertion, not a process crash.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| -> anyhow::Result<()> {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(async {
                    let root = support::tempdir()?;
                    let data = support::tempdir()?;
                    let engine = LibrqbitEngine::with_options(
                        data.path().into(),
                        support::local_options(),
                        vec![],
                    )
                    .await?;
                    let valid = support::torrent_bytes(&[7; 16384], false);
                    for depth in [140, 20000] {
                        let mut metadata = b"d9:extension".to_vec();
                        metadata.extend(vec![b'l'; depth]);
                        metadata.extend(b"i0e");
                        metadata.extend(vec![b'e'; depth]);
                        metadata.extend(&valid[1..]);
                        let path = root.path().join("deep.torrent");
                        std::fs::write(&path, metadata)?;
                        let result = engine.inspect(&path).await;
                        anyhow::ensure!(
                            matches!(
                                result,
                                Err(flowtorrent::error::AppError::InvalidMetadata(_))
                            ),
                            "deep untrusted extension was accepted"
                        );
                    }
                    Ok(())
                })
        })?
        .join()
        .map_err(|_| anyhow::anyhow!("metadata fixture thread panicked"))?
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn complete_owned_file_restores_while_an_ordinary_external_reader_stays_open()
-> anyhow::Result<()> {
    use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};
    const SHARE_READ: u32 = 1;
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let store = Arc::new(SessionStore::new(paths.clone()));
    let mut handle = lifecycle::start(root.path(), store.clone()).await?;
    let payload = support::payload();
    let metadata = support::torrent_bytes(&payload, false);
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
    .await?;
    lifecycle::stop(handle).await?;
    let destination = paths.download_dir.join("fixture.bin");
    std::fs::write(&destination, &payload)?;
    let reader = OpenOptions::new()
        .read(true)
        .share_mode(SHARE_READ)
        .open(&destination)?;
    let mut restored = lifecycle::start(root.path(), store).await?;
    let state = lifecycle::until(&mut restored, |s| {
        s.torrents
            .first()
            .is_some_and(|row| matches!(row.state, TransferState::Completed | TransferState::Error))
    })
    .await?;
    lifecycle::stop(restored).await?;
    drop(reader);
    assert_eq!(
        state.torrents[0].state,
        TransferState::Completed,
        "{:?}",
        state.torrents[0].error
    );
    assert_eq!(std::fs::read(destination)?, payload);
    Ok(())
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retry_after_real_write_sharing_error_honors_an_immediate_pause() -> anyhow::Result<()> {
    use flowtorrent::{
        app::AppController,
        settings::{Settings, SettingsStore},
    };
    use std::{fs::OpenOptions, num::NonZeroU32, os::windows::fs::OpenOptionsExt};
    let root = support::tempdir()?;
    let source = support::tempdir()?;
    let payload: Vec<_> = support::payload()
        .into_iter()
        .cycle()
        .take(64 * 1024 * 1024)
        .collect();
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
    let engine = LibrqbitEngine::with_options(
        paths.download_dir.clone(),
        support::local_options(),
        vec![peer],
    )
    .await?;
    let mut handle = AppController::spawn(
        Box::new(engine),
        Arc::new(SessionStore::new(paths.clone())),
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
    handle.commands.send(AppCommand::Pause(id)).await?;
    lifecycle::until(&mut handle, |s| {
        s.torrents[0].state == TransferState::Paused
    })
    .await?;
    let reader = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(paths.download_dir.join("fixture.bin"))?;
    handle.commands.send(AppCommand::Resume(id)).await?;
    lifecycle::until(&mut handle, |s| s.torrents[0].state == TransferState::Error).await?;
    drop(reader);
    handle.commands.send(AppCommand::Retry(id)).await?;
    handle.commands.send(AppCommand::Pause(id)).await?;
    // Always stop the owned sessions before returning an expected RED result.
    let result = lifecycle::until(&mut handle, |s| {
        s.torrents[0].state == TransferState::Paused
    })
    .await;
    lifecycle::stop(handle).await?;
    seeder.stop().await;
    let state = result?;
    assert_eq!(state.torrents[0].desired_state, DesiredState::Paused);
    Ok(())
}
