#[allow(dead_code)]
mod support;
use anyhow::{Context, Result};
use flowtorrent::{
    domain::*,
    engine::{TorrentEngine, librqbit::LibrqbitEngine},
};
use std::{
    num::NonZeroU32,
    time::{Duration, Instant},
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_torrents_share_global_limits() -> Result<()> {
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    let payload = support::payload().repeat(4);
    let public = support::torrent_bytes(&payload, false);
    let private = support::torrent_bytes(&payload, true);
    let (seed, peer) = support::seed(source.path(), &public, &payload).await?;
    let other = seed
        .add_torrent(
            librqbit::AddTorrent::from_bytes(private.clone()),
            Some(librqbit::AddTorrentOptions {
                overwrite: true,
                ..Default::default()
            }),
        )
        .await?
        .into_handle()
        .context("second seed")?;
    tokio::time::timeout(Duration::from_secs(15), other.wait_until_completed()).await??;
    let mut engine =
        LibrqbitEngine::with_options(target.path().into(), support::local_options(), vec![peer])
            .await?;
    engine
        .set_limits(TransferLimits {
            download_bps: NonZeroU32::new(65_536),
            upload_bps: None,
        })
        .await?;
    for (index, metadata) in [public, private].into_iter().enumerate() {
        let path = source.path().join(format!("{index}.torrent"));
        std::fs::write(&path, metadata)?;
        let folder = target.path().join(index.to_string());
        std::fs::create_dir(&folder)?;
        let preview = engine.inspect(&path).await?;
        engine
            .add(
                &preview,
                &AddRequest::new(preview.id, folder, preview.default_selection()),
                AddMode::New,
            )
            .await?;
    }
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if engine
                .snapshot()
                .await?
                .iter()
                .map(|s| s.checked_downloaded_bytes)
                .sum::<u64>()
                >= 131_072
            {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await??;
    let baseline = engine
        .snapshot()
        .await?
        .iter()
        .map(|s| s.checked_downloaded_bytes)
        .sum::<u64>();
    let start = Instant::now();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let rows = engine.snapshot().await?;
    let delta = rows.iter().map(|s| s.checked_downloaded_bytes).sum::<u64>() - baseline;
    assert!(delta >= 16_384, "positive transfer control: {delta}");
    assert!(
        delta as f64 <= start.elapsed().as_secs_f64() * 65_536.0 + 49_152.0,
        "aggregate useful bytes exceeded global limit: {delta}"
    );
    for row in rows {
        engine.pause(row.id).await?;
    }
    let paused = engine.snapshot().await?;
    tokio::time::sleep(Duration::from_millis(250)).await;
    for row in engine.snapshot().await? {
        assert_eq!(row.state, TransferState::Paused);
        assert_eq!((row.download_bps, row.upload_bps), (0, 0));
        assert_eq!(
            row.checked_downloaded_bytes,
            paused
                .iter()
                .find(|p| p.id == row.id)
                .unwrap()
                .checked_downloaded_bytes
        );
    }
    engine.shutdown().await?;
    seed.stop().await;
    Ok(())
}
