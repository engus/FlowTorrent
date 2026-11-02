#[allow(dead_code)]
mod support;
use flowtorrent::{
    domain::*,
    engine::{TorrentEngine, librqbit::LibrqbitEngine},
};
use std::time::Duration;

async fn settled(engine: &LibrqbitEngine, finished: bool) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let row = engine.snapshot().await?.remove(0);
            if row.state != TransferState::Checking
                && (!finished || row.checked_downloaded_bytes == row.selected_bytes)
            {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn paused_selection_reenables_completed_and_empty_files_without_losing_bytes()
-> anyhow::Result<()> {
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    // The shared piece crosses first.bin/second.bin, exercising boundary data.
    let first = vec![41; 20_000];
    let second = vec![72; 100_000];
    let metadata = support::multi_torrent_bytes(&first, &second);
    std::fs::create_dir(source.path().join("bundle"))?;
    std::fs::write(source.path().join("bundle/first.bin"), &first)?;
    std::fs::write(source.path().join("bundle/second.bin"), &second)?;
    let seed =
        librqbit::Session::new_with_opts(source.path().into(), support::local_options()).await?;
    let seeding = seed
        .add_torrent(
            librqbit::AddTorrent::from_bytes(metadata.clone()),
            Some(librqbit::AddTorrentOptions {
                overwrite: true,
                ..Default::default()
            }),
        )
        .await?
        .into_handle()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), seeding.wait_until_completed()).await??;
    let peer = seed.listen_addr().unwrap();
    let path = source.path().join("input.torrent");
    std::fs::write(&path, metadata)?;
    let mut engine =
        LibrqbitEngine::with_options(target.path().into(), support::local_options(), vec![peer])
            .await?;
    let preview = engine.inspect(&path).await?;
    let id = engine
        .add(
            &preview,
            &AddRequest::new(preview.id, target.path().into(), vec![FileId(0)]),
            AddMode::New,
        )
        .await?;
    settled(&engine, true).await?;
    assert_eq!(
        std::fs::read(target.path().join("bundle/first.bin"))?,
        first
    );
    let before = engine.details(id).await?.unwrap();
    assert!(!before.files[1].selected);
    assert!(before.files[1].checked_bytes < second.len() as u64);
    // Selection API is deliberately paused-only, including already complete torrents.
    assert!(engine.set_file_selection(id, &[FileId(1)]).await.is_err());
    engine.pause(id).await?;
    assert!(engine.set_file_selection(id, &[FileId(9)]).await.is_err());
    assert!(
        engine
            .set_file_selection(id, &[FileId(0), FileId(0)])
            .await
            .is_err()
    );
    engine.set_file_selection(id, &[]).await?;
    let row = engine.snapshot().await?.remove(0);
    assert_eq!(row.state, TransferState::NoFilesSelected);
    assert_eq!((row.selected_bytes, row.checked_downloaded_bytes), (0, 0));
    engine
        .set_file_selection(id, &[FileId(0), FileId(1)])
        .await?;
    assert_eq!(
        engine.details(id).await?.unwrap().files[0].checked_bytes,
        first.len() as u64
    );
    engine.resume(id).await?;
    settled(&engine, true).await?;
    assert_eq!(
        std::fs::read(target.path().join("bundle/first.bin"))?,
        first
    );
    assert_eq!(
        std::fs::read(target.path().join("bundle/second.bin"))?,
        second
    );
    engine.pause(id).await?;
    engine.set_file_selection(id, &[]).await?;
    engine.shutdown().await?;
    // Empty restore reuses only the already owned, safely checked files.
    let mut restarted =
        LibrqbitEngine::with_options(target.path().into(), support::local_options(), vec![])
            .await?;
    restarted
        .add(
            &preview,
            &AddRequest::new(id, target.path().into(), vec![]),
            AddMode::RestorePaused,
        )
        .await?;
    settled(&restarted, false).await?;
    assert_eq!(
        restarted.snapshot().await?[0].state,
        TransferState::NoFilesSelected
    );
    assert!(
        restarted
            .details(id)
            .await?
            .unwrap()
            .files
            .iter()
            .all(|f| !f.selected)
    );
    restarted.shutdown().await?;
    seed.stop().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabling_during_transfer_stops_unwanted_progress_and_reenable_reuses_checked_data()
-> anyhow::Result<()> {
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    let first = vec![17; 262_144];
    let second = vec![83; 262_144];
    let metadata = support::multi_torrent_bytes(&first, &second);
    std::fs::create_dir(source.path().join("bundle"))?;
    std::fs::write(source.path().join("bundle/first.bin"), &first)?;
    std::fs::write(source.path().join("bundle/second.bin"), &second)?;
    let seed =
        librqbit::Session::new_with_opts(source.path().into(), support::local_options()).await?;
    let seeding = seed
        .add_torrent(
            librqbit::AddTorrent::from_bytes(metadata.clone()),
            Some(librqbit::AddTorrentOptions {
                overwrite: true,
                ..Default::default()
            }),
        )
        .await?
        .into_handle()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), seeding.wait_until_completed()).await??;
    let mut engine = LibrqbitEngine::with_options(
        target.path().into(),
        support::local_options(),
        vec![seed.listen_addr().unwrap()],
    )
    .await?;
    engine
        .set_limits(TransferLimits {
            download_bps: std::num::NonZeroU32::new(65_536),
            upload_bps: None,
        })
        .await?;
    let path = source.path().join("input.torrent");
    std::fs::write(&path, metadata)?;
    let preview = engine.inspect(&path).await?;
    let id = engine
        .add(
            &preview,
            &AddRequest::new(
                preview.id,
                target.path().into(),
                preview.default_selection(),
            ),
            AddMode::New,
        )
        .await?;
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let row = engine.snapshot().await?.remove(0);
            if row.checked_downloaded_bytes >= 32_768
                && row.checked_downloaded_bytes < row.selected_bytes
            {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    engine.pause(id).await?;
    let checked = engine.details(id).await?.unwrap().files[1].checked_bytes;
    engine.set_file_selection(id, &[FileId(0)]).await?;
    engine.resume(id).await?;
    settled(&engine, true).await?;
    assert_eq!(
        engine.details(id).await?.unwrap().files[1].checked_bytes,
        checked
    );
    assert_eq!(
        std::fs::read(target.path().join("bundle/first.bin"))?,
        first
    );
    engine.pause(id).await?;
    engine
        .set_file_selection(id, &[FileId(0), FileId(1)])
        .await?;
    assert_eq!(
        engine.details(id).await?.unwrap().files[0].checked_bytes,
        first.len() as u64
    );
    engine.resume(id).await?;
    settled(&engine, true).await?;
    assert_eq!(
        std::fs::read(target.path().join("bundle/second.bin"))?,
        second
    );
    engine.shutdown().await?;
    seed.stop().await;
    Ok(())
}
