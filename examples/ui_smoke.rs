//! Full UI with an isolated persistent registry and a loopback-only seeder.
//! Usage: ui_smoke <absolute-owned-fixture-directory> [--open]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#[path = "../tests/support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[path = "../tests/support/mod.rs"]
#[allow(dead_code)]
mod support;
use flowtorrent::{
    app::{AppController, commands::AppCommand, storage::SessionStore},
    engine::librqbit::LibrqbitEngine,
    settings::SettingsStore,
    ui::{UiController, i18n::Locale},
};
use std::{path::PathBuf, sync::Arc, time::Duration};

fn main() -> anyhow::Result<()> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("Provide an owned fixture directory"))?,
    );
    anyhow::ensure!(root.is_absolute(), "Fixture directory must be absolute");
    std::fs::create_dir_all(&root)?;
    let root = root.canonicalize()?;
    let open = std::env::args_os().skip(2).any(|arg| arg == "--open");
    let locale = if std::env::args_os().any(|arg| arg == "--en") {
        Locale::En
    } else {
        Locale::Ru
    };
    let dark = std::env::args_os().any(|arg| arg == "--dark");
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let backend = runtime.spawn(async move {
        let source = support::tempdir()?;
        let payload = support::payload();
        let metadata = support::torrent_bytes(&payload, false);
        let (seeder, peer) = support::seed(source.path(), &metadata, &payload).await?;
        let paths = lifecycle::paths(&root);
        std::fs::create_dir_all(&paths.download_dir)?;
        let engine = LibrqbitEngine::with_options(
            paths.download_dir.clone(),
            support::local_options(),
            vec![peer],
        )
        .await?;
        let handle = AppController::spawn(
            Box::new(engine),
            Arc::new(SessionStore::new(paths.clone())),
            Arc::new(SettingsStore::new(
                paths.config_file.clone(),
                paths.download_dir.clone(),
            )),
            paths,
        );
        if open {
            let input = root.join("input.torrent");
            std::fs::write(&input, &metadata)?;
            handle.commands.send(AppCommand::OpenTorrent(input)).await?;
        }
        sender
            .send(handle)
            .map_err(|_| anyhow::anyhow!("UI receiver closed"))?;
        let _ = stopped.await;
        seeder.stop().await;
        drop(source);
        Ok::<_, anyhow::Error>(())
    });
    let handle = receiver.recv()?;
    let commands = handle.commands.clone();
    let mut observer = handle.snapshots.clone();
    let result = (|| -> anyhow::Result<()> {
        let _entered = runtime.enter();
        let ui = UiController::new_with_locale(handle, locale)?;
        ui.preview_theme(dark);
        ui.window().show()?;
        slint::run_event_loop()?;
        Ok(())
    })();
    runtime.spawn(async move {
        let _ = commands.send(AppCommand::Shutdown).await;
    });
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(20), async {
            while observer.changed().await.is_ok() {}
        })
        .await
    })?;
    let _ = stop.send(());
    runtime.block_on(backend)??;
    runtime.shutdown_timeout(Duration::from_secs(5));
    result
}
