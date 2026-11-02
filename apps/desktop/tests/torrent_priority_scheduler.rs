// Exercise the exact vendored queue, including its internal behavioral tests.
#[path = "../../../vendor/librqbit/src/download_queue.rs"]
mod download_queue;

use download_queue::DownloadQueue;
use std::{num::NonZeroU32, sync::Arc, time::Duration};
#[tokio::test]
async fn weighted_admission_is_per_torrent_and_cancellation_and_unlimited_release_waiters() {
    let queue = Arc::new(DownloadQueue::new(NonZeroU32::new(65_536)));
    queue.reserve(0, 2, NonZeroU32::new(65_536).unwrap()).await;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut jobs = Vec::new();
    for (id, weight, peers) in [(1, 4, 8), (2, 1, 32)] {
        for _ in 0..peers {
            let queue = queue.clone();
            let tx = tx.clone();
            jobs.push(tokio::spawn(async move {
                loop {
                    queue
                        .reserve(id, weight, NonZeroU32::new(1024).unwrap())
                        .await;
                    if tx.send(id).is_err() {
                        break;
                    }
                }
            }));
        }
    }
    let mut high = 0;
    let mut low = 0;
    for _ in 0..50 {
        match tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            1 => high += 1,
            _ => low += 1,
        }
    }
    assert!(
        high >= 38 && low >= 8,
        "peer count changed bandwidth preference: {high}/{low}"
    );
    for job in jobs {
        job.abort();
        let _ = job.await;
    }
    let waiter = {
        let queue = queue.clone();
        tokio::spawn(async move { queue.reserve(3, 1, NonZeroU32::new(65_536).unwrap()).await })
    };
    tokio::task::yield_now().await;
    waiter.abort();
    let _ = waiter.await;
    let next = {
        let queue = queue.clone();
        tokio::spawn(async move { queue.reserve(4, 2, NonZeroU32::new(65_536).unwrap()).await })
    };
    tokio::task::yield_now().await;
    queue.set_rate(None);
    tokio::time::timeout(Duration::from_millis(200), next)
        .await
        .unwrap()
        .unwrap();
    queue
        .reserve(5, 1, NonZeroU32::new(u32::MAX).unwrap())
        .await;
}
