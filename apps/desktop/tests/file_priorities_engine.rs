#[allow(dead_code)]
mod support;
use flowtorrent::{
    domain::*,
    engine::{TorrentEngine, librqbit::LibrqbitEngine},
};
use std::{num::NonZeroU32, time::Duration};
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn priorities_change_real_download_order_and_all_selected_files_still_complete()
-> anyhow::Result<()> {
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    let first = vec![31; 1_048_576];
    let second = vec![71; 1_048_576];
    let metadata = support::multi_torrent_bytes(&first, &second);
    std::fs::create_dir(source.path().join("bundle"))?;
    std::fs::write(source.path().join("bundle/first.bin"), &first)?;
    std::fs::write(source.path().join("bundle/second.bin"), &second)?;
    let seed =
        librqbit::Session::new_with_opts(source.path().into(), support::local_options()).await?;
    let seeded = seed
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
    tokio::time::timeout(Duration::from_secs(15), seeded.wait_until_completed()).await??;
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
    let path = source.path().join("input.torrent");
    std::fs::write(&path, metadata)?;
    let preview = engine.inspect(&path).await?;
    let mut request = AddRequest::new(
        preview.id,
        target.path().into(),
        preview.default_selection(),
    );
    request.file_priorities = vec![
        FilePriorityOverride {
            file: FileId(0),
            priority: FilePriority::Low,
        },
        FilePriorityOverride {
            file: FileId(1),
            priority: FilePriority::High,
        },
    ];
    let id = engine.add(&preview, &request, AddMode::New).await?;
    assert_eq!(
        engine.details(id).await?.unwrap().files[1].priority,
        FilePriority::High
    );
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let d = engine.details(id).await?.unwrap();
            if d.files.iter().map(|f| f.checked_bytes).sum::<u64>() >= 65_536 {
                assert!(
                    d.files[1].checked_bytes > d.files[0].checked_bytes,
                    "High did not lead filename-first Low: {d:?}"
                );
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    assert!(
        engine.set_file_priorities(id, &[]).await.is_err(),
        "live updates must require pause"
    );
    engine.pause(id).await?;
    assert!(
        engine
            .set_file_priorities(
                id,
                &[FilePriorityOverride {
                    file: FileId(99),
                    priority: FilePriority::High
                }]
            )
            .await
            .is_err()
    );
    engine
        .set_file_priorities(
            id,
            &[
                FilePriorityOverride {
                    file: FileId(0),
                    priority: FilePriority::High,
                },
                FilePriorityOverride {
                    file: FileId(1),
                    priority: FilePriority::Low,
                },
            ],
        )
        .await?;
    let before = engine.details(id).await?.unwrap();
    engine.resume(id).await?;
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let d = engine.details(id).await?.unwrap();
            if d.files[0].checked_bytes >= before.files[0].checked_bytes + 131_072 {
                assert!(
                    d.files[0].checked_bytes - before.files[0].checked_bytes
                        > d.files[1].checked_bytes - before.files[1].checked_bytes,
                    "changed High did not lead: {d:?}"
                );
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    tokio::time::timeout(Duration::from_secs(25), async {
        loop {
            let r = engine.snapshot().await?.remove(0);
            if r.checked_downloaded_bytes == r.selected_bytes {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    assert_eq!(
        std::fs::read(target.path().join("bundle/first.bin"))?,
        first
    );
    assert_eq!(
        std::fs::read(target.path().join("bundle/second.bin"))?,
        second
    );
    engine.shutdown().await?;
    seed.stop().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_high_pieces_do_not_starve_available_low_and_native_orders_are_validated()
-> anyhow::Result<()> {
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    let first = vec![21; 262_144];
    let second = vec![89; 262_144];
    let metadata = support::multi_torrent_bytes(&first, &second);
    std::fs::create_dir(source.path().join("bundle"))?;
    std::fs::write(source.path().join("bundle/first.bin"), &first)?;
    std::fs::write(source.path().join("bundle/second.bin"), [])?;
    let seed =
        librqbit::Session::new_with_opts(source.path().into(), support::local_options()).await?;
    let seeded = seed
        .add_torrent(
            librqbit::AddTorrent::from_bytes(metadata.clone()),
            Some(librqbit::AddTorrentOptions {
                overwrite: true,
                only_files: Some(vec![0]),
                paused: true,
                ..Default::default()
            }),
        )
        .await?
        .into_handle()
        .unwrap();
    for order in [vec![], vec![0], vec![0, 0], vec![0, 2]] {
        assert!(seeded.set_file_priority_order(&order).is_err());
    }
    seeded.set_file_priority_order(&[1, 0])?;
    tokio::time::timeout(Duration::from_secs(15), async {
        while !matches!(seeded.stats().state, librqbit::TorrentStatsState::Paused) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    seed.unpause(&seeded).await?;
    tokio::time::timeout(Duration::from_secs(15), seeded.wait_until_completed()).await??;
    assert_eq!(
        seeded.stats().file_progress[1],
        0,
        "source must not own High pieces"
    );
    assert!(seeded.set_file_priority_order(&[0, 1]).is_err());
    let mut engine = LibrqbitEngine::with_options(
        target.path().into(),
        support::local_options(),
        vec![seed.listen_addr().unwrap()],
    )
    .await?;
    let path = source.path().join("input.torrent");
    std::fs::write(&path, metadata)?;
    let preview = engine.inspect(&path).await?;
    let request = AddRequest::new(
        preview.id,
        target.path().into(),
        preview.default_selection(),
    )
    .with_file_priorities(vec![
        FilePriorityOverride {
            file: FileId(0),
            priority: FilePriority::Low,
        },
        FilePriorityOverride {
            file: FileId(1),
            priority: FilePriority::High,
        },
    ]);
    let id = engine.add(&preview, &request, AddMode::New).await?;
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let details = engine.details(id).await?.unwrap();
            if details.files[0].checked_bytes == first.len() as u64 {
                assert!(
                    details.files[1].checked_bytes < second.len() as u64,
                    "positive missing-High control invalid"
                );
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    assert_eq!(
        std::fs::read(target.path().join("bundle/first.bin"))?,
        first
    );
    engine.shutdown().await?;
    seed.stop().await;
    Ok(())
}
