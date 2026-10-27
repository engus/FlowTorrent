mod support;

use anyhow::{Result, bail};
use flowtorrent::{
    domain::{AddMode, AddRequest, TransferState},
    engine::{TorrentEngine, librqbit::LibrqbitEngine},
};
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn controlled_swarm_downloads_known_bytes() -> Result<()> {
    download_known_bytes(false).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_v1_torrent_downloads_known_bytes() -> Result<()> {
    download_known_bytes(true).await
}

async fn download_known_bytes(private: bool) -> Result<()> {
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    let bytes = support::payload();
    let metadata = support::torrent_bytes(&bytes, private);
    let torrent_file = source.path().join("sample.torrent");
    std::fs::write(&torrent_file, &metadata)?;
    let (seeder, peer) = support::seed(source.path(), &metadata, &bytes).await?;
    let mut engine =
        LibrqbitEngine::with_options(target.path().into(), support::local_options(), vec![peer])
            .await?;
    let preview = engine.inspect(&torrent_file).await?;
    let id = engine
        .add(
            &preview,
            &AddRequest::new(
                preview.id,
                target.path().to_owned(),
                preview.files.iter().map(|f| f.id).collect(),
            ),
            AddMode::New,
        )
        .await?;
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let rows = engine.snapshot().await?;
            let row = rows
                .iter()
                .find(|r| r.id == id)
                .expect("registered download");
            if let Some(error) = &row.error {
                bail!("download error: {error:?}");
            }
            if row.checked_downloaded_bytes == bytes.len() as u64 {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await??;
    assert_eq!(std::fs::read(target.path().join("fixture.bin"))?, bytes);
    #[cfg(windows)]
    {
        use std::{io::Read, os::windows::fs::OpenOptionsExt};
        // Match ordinary archive/hash readers, which allow other readers but
        // reject an outstanding write handle even after transfer completes.
        let mut reader = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ)
            .open(target.path().join("fixture.bin"))?;
        let mut actual = Vec::new();
        reader.read_to_end(&mut actual)?;
        assert_eq!(actual, bytes);
    }
    engine.shutdown().await?;
    seeder.stop().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selecting_second_file_uses_original_index_and_selected_progress() -> Result<()> {
    use librqbit::{AddTorrent, AddTorrentOptions, Session};
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    let first = vec![19; 32_768];
    let second = vec![43; 65_536];
    std::fs::write(source.path().join("first.bin"), &first)?;
    std::fs::write(source.path().join("second.bin"), &second)?;
    let metadata = support::multi_torrent_bytes(&first, &second);
    let path = source.path().join("selection.torrent");
    std::fs::write(&path, &metadata)?;
    let seeder = Session::new_with_opts(source.path().into(), support::local_options()).await?;
    let handle = seeder
        .add_torrent(
            AddTorrent::from_bytes(metadata),
            Some(AddTorrentOptions {
                overwrite: true,
                output_folder: Some(source.path().to_str().unwrap().into()),
                ..Default::default()
            }),
        )
        .await?
        .into_handle()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), handle.wait_until_completed()).await??;
    let mut engine = LibrqbitEngine::with_options(
        target.path().into(),
        support::local_options(),
        vec![seeder.listen_addr().unwrap()],
    )
    .await?;
    let preview = engine.inspect(&path).await?;
    let second_id = preview.files[1].id;
    let id = engine
        .add(
            &preview,
            &AddRequest::new(preview.id, target.path().into(), vec![second_id]),
            AddMode::New,
        )
        .await?;
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let row = engine.snapshot().await?.remove(0);
            if row.state == TransferState::Seeding {
                assert_eq!(row.selected_bytes, 65_536);
                assert_eq!(row.checked_downloaded_bytes, 65_536);
                break;
            }
            if row.error.is_some() {
                bail!("selected file failed: {:?}", row.error);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    assert_eq!(
        std::fs::read(target.path().join("bundle/second.bin"))?,
        second
    );
    // librqbit opens empty placeholders for unselected files. Verify that no
    // unselected payload was downloaded; absence of a placeholder is not an
    // engine contract in 1.0.
    assert_eq!(
        std::fs::metadata(target.path().join("bundle/first.bin"))?.len(),
        0
    );
    engine.forget(id).await?;
    engine.shutdown().await?;
    seeder.stop().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inspecting_metadata_never_creates_payload_or_registers_download() -> Result<()> {
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    let path = source.path().join("preview.torrent");
    std::fs::write(&path, support::torrent_bytes(&support::payload(), false))?;
    let mut engine =
        LibrqbitEngine::with_options(target.path().into(), support::local_options(), vec![])
            .await?;
    let first = engine.inspect(&path).await?;
    let second = engine.inspect(&path).await?;
    assert_eq!(first.id, second.id);
    assert_eq!(first.name, "fixture.bin");
    assert_eq!(first.files.len(), 1);
    assert_eq!(first.files[0].length, 524_288);
    assert!(engine.snapshot().await?.is_empty());
    assert_eq!(std::fs::read_dir(target.path())?.count(), 0);
    let missing = engine
        .inspect(&source.path().join("missing.torrent"))
        .await
        .unwrap_err();
    assert_eq!(
        missing.user_error().code,
        flowtorrent::domain::ErrorCode::InvalidMetadata
    );
    engine.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restored_completed_payload_is_rechecked_and_stays_paused_until_resumed() -> Result<()> {
    let folder = support::tempdir()?;
    let bytes = support::payload();
    std::fs::write(folder.path().join("fixture.bin"), &bytes)?;
    let path = folder.path().join("restore.torrent");
    std::fs::write(&path, support::torrent_bytes(&bytes, false))?;
    let mut engine =
        LibrqbitEngine::with_options(folder.path().into(), support::local_options(), vec![])
            .await?;
    let preview = engine.inspect(&path).await?;
    let id = engine
        .add(
            &preview,
            &AddRequest::new(
                preview.id,
                folder.path().into(),
                preview.files.iter().map(|f| f.id).collect(),
            ),
            AddMode::RestorePaused,
        )
        .await?;
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let row = engine.snapshot().await?.remove(0);
            if row.state != TransferState::Checking {
                assert_eq!(row.state, TransferState::Completed);
                assert_eq!(row.checked_downloaded_bytes, 524_288);
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    engine.resume(id).await?;
    assert_eq!(engine.snapshot().await?[0].state, TransferState::Seeding);
    engine.pause(id).await?;
    assert_eq!(engine.snapshot().await?[0].state, TransferState::Completed);
    engine.forget(id).await?;
    assert!(engine.snapshot().await?.is_empty());
    assert_eq!(std::fs::read(folder.path().join("fixture.bin"))?, bytes);
    engine.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn download_limit_throttles_transfer_and_can_be_changed_live() -> Result<()> {
    use flowtorrent::domain::TransferLimits;
    use std::{num::NonZeroU32, time::Instant};
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    let bytes = support::payload();
    let metadata = support::torrent_bytes(&bytes, false);
    let path = source.path().join("limited.torrent");
    std::fs::write(&path, &metadata)?;
    let (seeder, peer) = support::seed(source.path(), &metadata, &bytes).await?;
    let mut engine =
        LibrqbitEngine::with_options(target.path().into(), support::local_options(), vec![peer])
            .await?;
    engine
        .set_limits(TransferLimits {
            download_bps: NonZeroU32::new(65_536),
            upload_bps: None,
        })
        .await?;
    let preview = engine.inspect(&path).await?;
    let id = engine
        .add(
            &preview,
            &AddRequest::new(preview.id, target.path().into(), vec![preview.files[0].id]),
            AddMode::New,
        )
        .await?;
    let started = Instant::now();
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let row = engine.snapshot().await?.remove(0);
            if row.checked_downloaded_bytes >= 131_072 {
                assert!(
                    started.elapsed() >= Duration::from_millis(750),
                    "limit must constrain useful bytes after its initial burst"
                );
                break;
            }
            if row.error.is_some() {
                bail!("limited transfer failed: {:?}", row.error);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    engine.pause(id).await?;
    let paused = engine.snapshot().await?.remove(0);
    assert_eq!(paused.state, TransferState::Paused);
    engine.set_limits(TransferLimits::default()).await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let still_paused = engine.snapshot().await?.remove(0);
    assert_eq!(still_paused.state, TransferState::Paused);
    assert_eq!(
        still_paused.checked_downloaded_bytes,
        paused.checked_downloaded_bytes
    );
    engine.resume(id).await?;
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let row = engine.snapshot().await?.remove(0);
            if row.state == TransferState::Seeding {
                break;
            }
            if row.error.is_some() {
                bail!("resume failed: {:?}", row.error);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    assert_eq!(std::fs::read(target.path().join("fixture.bin"))?, bytes);
    engine.shutdown().await?;
    seeder.stop().await;
    Ok(())
}

#[tokio::test]
async fn rates_below_supported_minimum_are_rejected_before_applying() -> Result<()> {
    use flowtorrent::domain::TransferLimits;
    use std::num::NonZeroU32;
    let target = support::tempdir()?;
    let mut engine =
        LibrqbitEngine::with_options(target.path().into(), support::local_options(), vec![])
            .await?;
    assert!(
        engine
            .set_limits(TransferLimits {
                download_bps: NonZeroU32::new(1_000),
                upload_bps: None
            })
            .await
            .is_err()
    );
    assert!(
        engine
            .set_limits(TransferLimits {
                download_bps: None,
                upload_bps: NonZeroU32::new(19_999)
            })
            .await
            .is_err()
    );
    engine
        .set_limits(TransferLimits {
            download_bps: NonZeroU32::new(20_000),
            upload_bps: NonZeroU32::new(20_000),
        })
        .await?;
    engine.shutdown().await?;
    Ok(())
}
