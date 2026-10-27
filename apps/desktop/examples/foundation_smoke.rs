//! Controlled GUI/engine probe. Synthetic data only; not the product add flow.
#[path = "../tests/support/mod.rs"]
#[allow(dead_code)]
mod support;

use flowtorrent::{
    domain::{AddMode, AddRequest, TransferLimits},
    engine::{TorrentEngine, librqbit::LibrqbitEngine},
};
use slint::ComponentHandle;
use std::{
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

include!(concat!(env!("OUT_DIR"), "/probe.rs"));

fn main() -> anyhow::Result<()> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let window = ProbeWindow::new()?;
    let weak = window.as_weak();
    let stopping = Arc::new(AtomicBool::new(false));
    let worker_stop = stopping.clone();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let worker = runtime.spawn(async move {
        let source = support::tempdir()?;
        let target = support::tempdir()?;
        let bytes = support::payload();
        let metadata = support::torrent_bytes(&bytes, false);
        let path = source.path().join("probe.torrent");
        std::fs::write(&path, &metadata)?;
        let (seeder, peer) = support::seed(source.path(), &metadata, &bytes).await?;
        let mut engine = LibrqbitEngine::with_options(
            target.path().into(),
            support::local_options(),
            vec![peer],
        )
        .await?;
        engine
            .set_limits(TransferLimits {
                download_bps: NonZeroU32::new(65_536),
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
                    preview.files.iter().map(|f| f.id).collect(),
                ),
                AddMode::New,
            )
            .await?;
        while !worker_stop.load(Ordering::Relaxed) {
            let row = engine.snapshot().await?.remove(0);
            let progress = row.checked_downloaded_bytes as f32 / bytes.len() as f32;
            let status = format!(
                "Local swarm: {:?}\n{} / {} bytes\nTarget: {}",
                row.state,
                row.checked_downloaded_bytes,
                bytes.len(),
                target.path().display()
            );
            weak.upgrade_in_event_loop(move |window| {
                window.set_progress(progress);
                window.set_status(status.into());
            })?;
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        assert_eq!(std::fs::read(target.path().join("fixture.bin"))?, bytes);
        engine.shutdown().await?;
        seeder.stop().await;
        Ok::<_, anyhow::Error>(())
    });
    window.run()?;
    stopping.store(true, Ordering::Relaxed);
    runtime.block_on(worker)??;
    Ok(())
}
