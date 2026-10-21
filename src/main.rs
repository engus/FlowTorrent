#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use flowtorrent::{
    app::{AppController, commands::AppCommand, storage::SessionStore},
    engine::librqbit::LibrqbitEngine,
    paths::AppPaths,
    settings::SettingsStore,
    ui::{UiController, i18n::Locale},
};
use std::{path::PathBuf, sync::Arc, time::Duration};

fn main() {
    if let Err(error) = run() {
        let locale = Locale::from_system_tag(&sys_locale::get_locale().unwrap_or_default());
        rfd::MessageDialog::new()
            .set_title("FlowTorrent")
            .set_description(format!(
                "{}\n\n{error:#}",
                locale.text(
                    "Не удалось запустить FlowTorrent.",
                    "FlowTorrent could not start."
                )
            ))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
fn run() -> anyhow::Result<()> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let paths = AppPaths::discover()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    runtime.spawn(async move {
        let settings = Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        ));
        let store = Arc::new(SessionStore::new(paths.clone()));
        let result = LibrqbitEngine::new(paths.download_dir.clone())
            .await
            .map(|engine| AppController::spawn(Box::new(engine), store, settings, paths));
        let _ = sender.send(result);
    });
    let handle = receiver.recv()??;
    let commands = handle.commands.clone();
    let mut observer = handle.snapshots.clone();
    let arguments: Vec<_> = std::env::args_os()
        .skip(1)
        .filter(|arg| arg != "--" && !arg.to_string_lossy().starts_with("-psn_"))
        .map(PathBuf::from)
        .collect();
    let result = (|| -> anyhow::Result<()> {
        let _entered = runtime.enter();
        let ui = UiController::new(handle)?;
        let input = commands.clone();
        runtime.spawn(async move {
            for path in arguments {
                if input.send(AppCommand::OpenTorrent(path)).await.is_err() {
                    break;
                }
            }
        });
        ui.window().show()?;
        slint::run_event_loop()?;
        Ok(())
    })();
    let shutdown = commands.clone();
    runtime.spawn(async move {
        let _ = shutdown.send(AppCommand::Shutdown).await;
    });
    let stopped = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(20), async {
            while observer.changed().await.is_ok() {}
        })
        .await
    });
    runtime.shutdown_timeout(Duration::from_secs(5));
    result?;
    stopped?;
    Ok(())
}
