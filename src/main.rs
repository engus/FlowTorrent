#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use flowtorrent::{
    app::{AppController, commands::AppCommand, storage::SessionStore},
    engine::librqbit::LibrqbitEngine,
    paths::AppPaths,
    platform::{
        PlatformEvent, PlatformShell,
        instance::{InstanceGuard, InstanceRole, forward_open_to},
    },
    settings::SettingsStore,
    ui::{UiController, i18n::Locale},
};
use std::{path::PathBuf, rc::Rc, sync::Arc, time::Duration};

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
    let paths = AppPaths::discover()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let current_dir = std::env::current_dir()?;
    let arguments: Vec<_> = std::env::args_os()
        .skip(1)
        .filter(|arg| arg != "--" && !arg.to_string_lossy().starts_with("-psn_"))
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                current_dir.join(path)
            }
        })
        .collect();
    let mut instance = match InstanceGuard::acquire(&paths)? {
        InstanceRole::Primary(instance) => instance,
        InstanceRole::Secondary => {
            runtime.block_on(forward_open_to(&paths, &arguments))?;
            runtime.shutdown_timeout(Duration::from_secs(1));
            return Ok(());
        }
    };
    let incoming = {
        let _entered = runtime.enter();
        instance.listen()?
    };
    let (native_events, native_receiver) = std::sync::mpsc::channel();
    anyhow::ensure!(
        arguments.len() <= 64,
        "Open at most 64 torrent files per launch"
    );
    if !arguments.is_empty() {
        native_events.send(PlatformEvent::OpenTorrentPaths(arguments))?;
    }
    runtime.spawn(flowtorrent::platform::forward_instance_events(
        incoming,
        native_events.clone(),
    ));
    let interrupt = native_events.clone();
    runtime.spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = interrupt.send(PlatformEvent::Quit);
        }
    });
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let notice_path = paths.data_dir.join("shell.json");
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
    let result = (|| -> anyhow::Result<()> {
        let _entered = runtime.enter();
        let ui = UiController::new(handle)?;
        ui.window().show()?;
        let shell = Rc::new(PlatformShell::attach(ui.window(), native_events)?);
        ui.enable_background(shell, native_receiver, notice_path)?;
        slint::run_event_loop_until_quit()?;
        Ok(())
    })();
    let shutdown = commands.clone();
    runtime.spawn(async move {
        let _ = shutdown.send(AppCommand::Shutdown).await;
    });
    runtime.block_on(instance.stop());
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
