//! Full UI with an isolated persistent registry and a loopback-only seeder.
//! Usage: ui_smoke <absolute-owned-fixture-directory> [--open] [--compact | --wide]
//! A native test bundle may supply FLOWTORRENT_UI_SMOKE_ROOT, _BACKGROUND=1 and
//! _COMPACT=1 via LSEnvironment so its executable stays inside the .app bundle.
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
    platform::{
        PlatformEvent, PlatformShell,
        instance::{InstanceGuard, InstanceRole, forward_open_to},
    },
    settings::SettingsStore,
    ui::{UiController, i18n::Locale},
};
use std::{path::PathBuf, rc::Rc, sync::Arc, time::Duration};

fn main() -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("warn,i_slint_core=debug")
        .try_init();
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .or_else(|| std::env::var_os("FLOWTORRENT_UI_SMOKE_ROOT"))
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
    let background = std::env::args_os().any(|arg| arg == "--background")
        || std::env::var_os("FLOWTORRENT_UI_SMOKE_BACKGROUND").is_some_and(|v| v == "1");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let (native_events, native_receiver) = std::sync::mpsc::channel();
    let mut instance = if background {
        let mut instance = match InstanceGuard::acquire(&lifecycle::paths(&root))? {
            InstanceRole::Primary(instance) => instance,
            InstanceRole::Secondary => {
                let paths = if open {
                    vec![root.join("input.torrent")]
                } else {
                    Vec::new()
                };
                runtime.block_on(forward_open_to(&lifecycle::paths(&root), &paths))?;
                return Ok(());
            }
        };
        let incoming = {
            let _entered = runtime.enter();
            instance.listen()?
        };
        runtime.spawn(flowtorrent::platform::forward_instance_events(
            incoming,
            native_events.clone(),
        ));
        let quit_marker = root.join("quit.fixture");
        let events = native_events.clone();
        runtime.spawn(async move {
            loop {
                if quit_marker.exists() {
                    let _ = events.send(PlatformEvent::Quit);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
        Some(instance)
    } else {
        None
    };
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let notice_path = root.join("state/shell.json");
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
        if std::env::args_os().any(|arg| arg == "--compact")
            || std::env::var_os("FLOWTORRENT_UI_SMOKE_COMPACT").is_some_and(|v| v == "1")
        {
            ui.window().set_size(slint::LogicalSize::new(800.0, 640.0));
        } else if std::env::args_os().any(|arg| arg == "--wide") {
            ui.window().set_size(slint::LogicalSize::new(1280.0, 720.0));
        }
        ui.window().show()?;
        if background {
            let shell = Rc::new(PlatformShell::attach(ui.window(), native_events)?);
            ui.enable_background(shell, native_receiver, notice_path)?;
            slint::run_event_loop_until_quit()?;
        } else {
            slint::run_event_loop()?;
        }
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
    if let Some(instance) = instance.as_mut() {
        runtime.block_on(instance.stop());
    }
    runtime.shutdown_timeout(Duration::from_secs(5));
    result
}
