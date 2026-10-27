use flowtorrent::{
    app::preview::{AddReservations, PreviewQueue},
    domain::TorrentId,
};
use std::{
    path::PathBuf,
    sync::{Arc, Barrier},
};
#[allow(dead_code)]
mod support;

#[test]
fn cancel_does_not_create_payload() -> anyhow::Result<()> {
    let target = tempfile::tempdir()?;
    let path = target.path().join("source.torrent");
    std::fs::write(&path, b"metadata is not read by the queue")?;
    let mut queue = PreviewQueue::default();
    queue.enqueue(path.clone());
    queue.enqueue(PathBuf::from("second.torrent"));
    assert_eq!(queue.current_path(), Some(path.as_path()));
    queue.cancel_current();
    assert_eq!(
        queue.current_path(),
        Some(std::path::Path::new("second.torrent"))
    );
    queue.cancel_current();
    assert!(queue.current_path().is_none());
    assert_eq!(std::fs::read_dir(target.path())?.count(), 1);
    Ok(())
}

#[test]
fn same_infohash_is_not_added_twice_concurrently() {
    let reservations = AddReservations::default();
    let barrier = Arc::new(Barrier::new(2));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let reservations = reservations.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                if let Some(reservation) = reservations.reserve(TorrentId([3; 20])) {
                    reservation.commit();
                    true
                } else {
                    false
                }
            })
        })
        .collect();
    let successes = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .filter(|success| *success)
        .count();
    assert_eq!(successes, 1);
    assert!(reservations.reserve(TorrentId([3; 20])).is_none());
}

#[test]
fn failed_confirmation_releases_reservation_for_retry() {
    let reservations = AddReservations::default();
    let id = TorrentId([8; 20]);
    {
        let _pending = reservations.reserve(id).unwrap();
        assert!(reservations.reserve(id).is_none());
    }
    assert!(reservations.reserve(id).is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn different_torrent_filenames_with_same_infohash_create_one_engine_job() -> anyhow::Result<()>
{
    use flowtorrent::{
        domain::{AddMode, AddRequest},
        engine::{TorrentEngine, librqbit::LibrqbitEngine},
    };
    let source = tempfile::tempdir()?;
    let target = tempfile::tempdir()?;
    let target_path = target.path().canonicalize()?;
    let first = source.path().join("first.torrent");
    let second = source.path().join("другое имя.torrent");
    let metadata = support::torrent_bytes(&support::payload(), false);
    std::fs::write(&first, &metadata)?;
    std::fs::write(&second, &metadata)?;
    let engine =
        LibrqbitEngine::with_options(target_path.clone(), support::local_options(), vec![]).await?;
    let first = engine.inspect(&first).await?;
    let second = engine.inspect(&second).await?;
    assert_eq!(first.id, second.id);
    let engine = Arc::new(tokio::sync::Mutex::new(engine));
    let mut additions = Vec::new();
    for preview in [first, second] {
        let engine = engine.clone();
        let folder = target_path.clone();
        additions.push(tokio::spawn(async move {
            let request = AddRequest::new(preview.id, folder, preview.default_selection());
            engine
                .lock()
                .await
                .add(&preview, &request, AddMode::New)
                .await
        }));
    }
    let second = additions.pop().unwrap().await??;
    let first = additions.pop().unwrap().await??;
    assert_eq!(first, second);
    let mut engine = engine.lock().await;
    assert_eq!(engine.snapshot().await?.len(), 1);
    assert_eq!(std::fs::read_dir(&target_path)?.count(), 1);
    engine.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn pure_v2_reports_unsupported_without_creating_payload() -> anyhow::Result<()> {
    use flowtorrent::{
        engine::{TorrentEngine, librqbit::LibrqbitEngine},
        error::AppError,
    };
    let source = tempfile::tempdir()?;
    let target = tempfile::tempdir()?;
    let path = source.path().join("v2.torrent");
    let value = serde_json::json!({"info":{"name":"file.bin","meta version":2,"piece length":16384,"file tree":{"":{"length":4,"pieces root":"\0".repeat(32)}}}});
    let mut bytes = Vec::new();
    librqbit_bencode::bencode_serialize_to_writer(&value, &mut bytes)?;
    std::fs::write(&path, bytes)?;
    let mut engine =
        LibrqbitEngine::with_options(target.path().into(), support::local_options(), vec![])
            .await?;
    let result = engine.inspect(&path).await;
    engine.shutdown().await?;
    assert!(
        matches!(result, Err(AppError::UnsupportedFormat)),
        "{result:?}"
    );
    assert_eq!(std::fs::read_dir(target.path())?.count(), 0);
    Ok(())
}

#[tokio::test]
async fn restoring_a_hardlink_cannot_modify_an_outside_file() -> anyhow::Result<()> {
    use flowtorrent::{
        domain::{AddMode, AddRequest},
        engine::{TorrentEngine, librqbit::LibrqbitEngine},
    };
    let root = tempfile::tempdir()?;
    let root_path = root.path().canonicalize()?;
    let target = root_path.join("target");
    std::fs::create_dir(&target)?;
    let outside = root_path.join("outside.bin");
    std::fs::write(&outside, b"keep unchanged")?;
    std::fs::hard_link(&outside, target.join("fixture.bin"))?;
    let path = root_path.join("restore.torrent");
    std::fs::write(&path, support::torrent_bytes(&support::payload(), false))?;
    let mut engine =
        LibrqbitEngine::with_options(target.clone(), support::local_options(), vec![]).await?;
    let preview = engine.inspect(&path).await?;
    let result = engine
        .add(
            &preview,
            &AddRequest::new(preview.id, target, preview.default_selection()),
            AddMode::RestorePaused,
        )
        .await;
    engine.shutdown().await?;
    assert!(
        result.is_err(),
        "restoration must reject multiply-linked files"
    );
    assert_eq!(std::fs::read(outside)?, b"keep unchanged");
    Ok(())
}
