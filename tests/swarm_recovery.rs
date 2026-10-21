//! Adapter contracts against real loopback peers; no host network settings change.
#[allow(dead_code)]
mod support;
use anyhow::{Context, Result, ensure};
use flowtorrent::{
    domain::*,
    engine::{TorrentEngine, librqbit::LibrqbitEngine},
};
use std::{num::NonZeroU32, time::Duration};

async fn wait_for(
    engine: &LibrqbitEngine,
    predicate: impl Fn(&TorrentSnapshot) -> bool,
) -> Result<TorrentSnapshot> {
    tokio::time::timeout(Duration::from_secs(45), async {
        loop {
            let row = engine.snapshot().await?.remove(0);
            ensure!(row.error.is_none(), "transfer failed: {:?}", row.error);
            if predicate(&row) {
                return Ok(row);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await?
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restart_rejects_one_corrupt_piece_and_reuses_every_other_piece() -> Result<()> {
    // Catches trusting file length or marking an unchecked piece as complete.
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    let payload = support::payload();
    let metadata = support::torrent_bytes(&payload, false);
    let path = source.path().join("input.torrent");
    std::fs::write(&path, &metadata)?;
    let mut engine =
        LibrqbitEngine::with_options(target.path().into(), support::local_options(), vec![])
            .await?;
    let preview = engine.inspect(&path).await?;
    let request = AddRequest::new(
        preview.id,
        target.path().into(),
        preview.default_selection(),
    );
    engine.add(&preview, &request, AddMode::New).await?;
    wait_for(&engine, |r| r.state != TransferState::Checking).await?;
    engine.pause(preview.id).await?;
    engine.shutdown().await?;
    let mut corrupt = payload.clone();
    corrupt[7 * 16_384 + 9] ^= 0xff;
    std::fs::write(target.path().join("fixture.bin"), &corrupt)?;
    let (seeder, peer) = support::seed(source.path(), &metadata, &payload).await?;
    let mut engine =
        LibrqbitEngine::with_options(target.path().into(), support::local_options(), vec![peer])
            .await?;
    engine
        .add(&preview, &request, AddMode::RestorePaused)
        .await?;
    let checked = wait_for(&engine, |r| r.state == TransferState::Paused).await?;
    assert_eq!(checked.checked_downloaded_bytes, 507_904);
    assert_eq!(std::fs::read(target.path().join("fixture.bin"))?, corrupt);
    engine.resume(preview.id).await?;
    wait_for(&engine, |r| r.state == TransferState::Seeding).await?;
    assert_eq!(std::fs::read(target.path().join("fixture.bin"))?, payload);
    engine.shutdown().await?;
    seeder.stop().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_file_crossing_piece_boundary_finishes_and_survives_restore() -> Result<()> {
    // Catches wrong selected progress and losing shared boundary bytes on restart.
    use librqbit::{AddTorrent, AddTorrentOptions, Session};
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    let first = vec![19; 20_000];
    let second = vec![43; 50_000];
    std::fs::write(source.path().join("first.bin"), &first)?;
    std::fs::write(source.path().join("second.bin"), &second)?;
    let metadata = support::multi_torrent_bytes(&first, &second);
    let path = source.path().join("input.torrent");
    std::fs::write(&path, &metadata)?;
    let seeder = Session::new_with_opts(source.path().into(), support::local_options()).await?;
    let seed = seeder
        .add_torrent(
            AddTorrent::from_bytes(metadata),
            Some(AddTorrentOptions {
                output_folder: Some(source.path().to_str().context("fixture path")?.into()),
                overwrite: true,
                ..Default::default()
            }),
        )
        .await?
        .into_handle()
        .context("seed")?;
    tokio::time::timeout(Duration::from_secs(15), seed.wait_until_completed()).await??;
    let mut engine = LibrqbitEngine::with_options(
        target.path().into(),
        support::local_options(),
        vec![seeder.listen_addr().context("listen")?],
    )
    .await?;
    let preview = engine.inspect(&path).await?;
    let request = AddRequest::new(preview.id, target.path().into(), vec![FileId(1)]);
    engine.add(&preview, &request, AddMode::New).await?;
    let row = wait_for(&engine, |r| r.state == TransferState::Seeding).await?;
    assert_eq!(
        (row.selected_bytes, row.checked_downloaded_bytes),
        (50_000, 50_000)
    );
    assert_eq!(
        std::fs::read(target.path().join("bundle/second.bin"))?,
        second
    );
    engine.shutdown().await?;
    seeder.stop().await;
    let mut engine =
        LibrqbitEngine::with_options(target.path().into(), support::local_options(), vec![])
            .await?;
    engine
        .add(&preview, &request, AddMode::RestorePaused)
        .await?;
    let row = wait_for(&engine, |r| r.state == TransferState::Completed).await?;
    assert_eq!(
        (row.selected_bytes, row.checked_downloaded_bytes),
        (50_000, 50_000)
    );
    engine.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broken_peer_connection_recovers_without_user_resume_or_losing_progress() -> Result<()> {
    // A relay drops established TCP streams and refuses connections while offline.
    // Catches treating a temporary peer outage as a terminal torrent error.
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    let payload = support::payload().repeat(8);
    let metadata = support::torrent_bytes(&payload, false);
    let path = source.path().join("input.torrent");
    std::fs::write(&path, &metadata)?;
    let (seeder, peer) = support::seed(source.path(), &metadata, &payload).await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (online, mut status) = tokio::sync::watch::channel(true);
    let relay = tokio::spawn(async move {
        let mut connections = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                changed = status.changed() => {
                    if changed.is_err() { break; }
                    if !*status.borrow() { connections.abort_all(); }
                }
                accepted = listener.accept() => {
                    let (mut incoming, _) = accepted?;
                    if *status.borrow() {
                        connections.spawn(async move {
                            let mut outgoing = tokio::net::TcpStream::connect(peer).await?;
                            tokio::io::copy_bidirectional(&mut incoming, &mut outgoing).await?;
                            Ok::<_, std::io::Error>(())
                        });
                    }
                }
                _ = connections.join_next(), if !connections.is_empty() => {}
            }
        }
        Ok::<_, std::io::Error>(())
    });
    let mut engine = LibrqbitEngine::with_options(
        target.path().into(),
        support::local_options(),
        vec![address],
    )
    .await?;
    engine
        .set_limits(TransferLimits {
            download_bps: NonZeroU32::new(131_072),
            upload_bps: None,
        })
        .await?;
    let preview = engine.inspect(&path).await?;
    engine
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
    let before = wait_for(&engine, |r| r.checked_downloaded_bytes >= 32_768).await?;
    online.send(false)?;
    let offline = wait_for(&engine, |r| r.state == TransferState::WaitingForSources).await?;
    assert!(offline.checked_downloaded_bytes >= before.checked_downloaded_bytes);
    assert!(offline.checked_downloaded_bytes < payload.len() as u64);
    online.send(true)?;
    engine.set_limits(TransferLimits::default()).await?;
    let complete = wait_for(&engine, |r| r.state == TransferState::Seeding).await?;
    assert_eq!(complete.checked_downloaded_bytes, 4_194_304);
    assert_eq!(std::fs::read(target.path().join("fixture.bin"))?, payload);
    engine.shutdown().await?;
    drop(online);
    relay.await??;
    seeder.stop().await;
    Ok(())
}
