#[allow(dead_code)]
mod support;
use flowtorrent::{
    domain::*,
    engine::{TorrentEngine, librqbit::LibrqbitEngine},
};
use std::{num::NonZeroU32, time::Duration};
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn torrent_weights_prefer_high_reverse_on_edit_and_both_payloads_complete()
-> anyhow::Result<()> {
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    let seed =
        librqbit::Session::new_with_opts(source.path().into(), support::local_options()).await?;
    let mut engine = LibrqbitEngine::with_options(
        target.path().into(),
        support::local_options(),
        vec![seed.listen_addr().unwrap()],
    )
    .await?;
    engine
        .set_limits(TransferLimits {
            download_bps: NonZeroU32::new(262_144),
            upload_bps: None,
        })
        .await?;
    let mut ids = Vec::new();
    let mut payloads = Vec::new();
    let mut destinations = Vec::new();
    for (n, priority) in [(1, FilePriority::High), (2, FilePriority::Low)] {
        let data = vec![n; 2 * 1024 * 1024];
        let dir = source.path().join(n.to_string());
        std::fs::create_dir(&dir)?;
        std::fs::write(dir.join("fixture.bin"), &data)?;
        let metadata = support::torrent_bytes(&data, true);
        let handle = seed
            .add_torrent(
                librqbit::AddTorrent::from_bytes(metadata.clone()),
                Some(librqbit::AddTorrentOptions {
                    output_folder: Some(dir.to_str().unwrap().into()),
                    overwrite: true,
                    ..Default::default()
                }),
            )
            .await?
            .into_handle()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(15), handle.wait_until_completed()).await??;
        let path = source.path().join(format!("{n}.torrent"));
        std::fs::write(&path, metadata)?;
        let preview = engine.inspect(&path).await?;
        let base = target.path().join(n.to_string());
        std::fs::create_dir(&base)?;
        let id = engine
            .add(
                &preview,
                &AddRequest::new(preview.id, base.clone(), preview.default_selection())
                    .with_priority(priority),
                AddMode::New,
            )
            .await?;
        ids.push(id);
        payloads.push(data);
        destinations.push(base);
    }
    // Allow initial token burst/connections to settle, compare equal ongoing intervals.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let before = sample(&engine, &ids).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let after = sample(&engine, &ids).await?;
    assert!(
        after[0] - before[0] > (after[1] - before[1]) * 2,
        "High did not get preference: {before:?} -> {after:?}"
    );
    for id in &ids {
        engine.pause(*id).await?;
    }
    engine
        .set_torrent_priority(ids[0], FilePriority::Low)
        .await?;
    engine
        .set_torrent_priority(ids[1], FilePriority::High)
        .await?;
    for id in &ids {
        engine.resume(*id).await?;
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
    let before = sample(&engine, &ids).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let after = sample(&engine, &ids).await?;
    assert!(
        after[1] - before[1] > (after[0] - before[0]) * 2,
        "Preference did not reverse: {before:?} -> {after:?}"
    );
    engine.set_limits(TransferLimits::default()).await?;
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if engine
                .snapshot()
                .await?
                .iter()
                .all(|r| r.state == TransferState::Seeding)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    for (dir, data) in destinations.iter().zip(payloads) {
        assert_eq!(std::fs::read(dir.join("fixture.bin"))?, data);
    }
    engine.shutdown().await?;
    seed.stop().await;
    Ok(())
}

async fn sample(engine: &LibrqbitEngine, ids: &[TorrentId]) -> anyhow::Result<[u64; 2]> {
    let rows = engine.snapshot().await?;
    Ok([
        rows.iter()
            .find(|r| r.id == ids[0])
            .unwrap()
            .checked_downloaded_bytes,
        rows.iter()
            .find(|r| r.id == ids[1])
            .unwrap()
            .checked_downloaded_bytes,
    ])
}
