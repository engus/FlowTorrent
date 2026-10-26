//! One-process component attribution; absolute numbers include preceding stages.
//! Usage: memory_breakdown <new-absolute-fixture-directory> [--relief]
//! `--relief` is an opt-in diagnostic experiment, never enabled in the app.
#[path = "resources/memory.rs"]
mod memory;
use flowtorrent::{
    app::{AppController, storage::SessionStore},
    engine::librqbit::LibrqbitEngine,
    paths::AppPaths,
    platform::PlatformShell,
    settings::SettingsStore,
    ui::{UiController, i18n::Locale},
};
use std::{path::PathBuf, rc::Rc, sync::Arc, time::Duration};

fn report(stage: &str) {
    let value = memory::sample(std::process::id()).unwrap();
    println!(
        "{stage}: resident={} footprint={:?} private={:?} cpu_ns={}",
        value.resident_bytes,
        value.physical_footprint_bytes,
        value.private_bytes,
        value.cpu_time_ns
    );
}
fn main() -> anyhow::Result<()> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("Supply a new absolute fixture directory"))?,
    );
    anyhow::ensure!(root.is_absolute(), "Absolute path required");
    std::fs::create_dir(&root)?;
    anyhow::ensure!(
        !cfg!(debug_assertions),
        "Use a release build for measurements"
    );
    let relief = std::env::args_os().skip(2).any(|arg| arg == "--relief");
    report("process");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    report("runtime");
    let paths = AppPaths {
        config_file: root.join("settings.json"),
        data_dir: root.clone(),
        metadata_dir: root.join("metadata"),
        manifest_file: root.join("session.json"),
        download_dir: root.join("downloads"),
    };
    std::fs::create_dir(&paths.download_dir)?;
    let engine = runtime.block_on(LibrqbitEngine::with_options(
        paths.download_dir.clone(),
        librqbit::SessionOptions {
            dht: None,
            listen: None,
            disable_local_service_discovery: true,
            disable_trackers: true,
            ..Default::default()
        },
        vec![],
    ))?;
    report("engine");
    let _entered = runtime.enter();
    let handle = AppController::spawn(
        Box::new(engine),
        Arc::new(SessionStore::new(paths.clone())),
        Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        )),
        paths,
    );
    let commands = handle.commands.clone();
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let ui = Rc::new(UiController::new_with_locale(handle, Locale::En)?);
    report("UI before show");
    ui.window().show()?;
    let (events, receiver) = std::sync::mpsc::channel();
    let shell = Rc::new(PlatformShell::attach(ui.window(), events)?);
    ui.enable_background(shell, receiver, root.join("shell.json"))?;
    report("show and shell");
    let hidden = ui.clone();
    slint::Timer::single_shot(Duration::from_secs(5), move || {
        report("painted");
        println!(
            "display_scale={} window_physical={}x{}",
            hidden.window().scale_factor(),
            hidden.window().size().width,
            hidden.window().size().height
        );

        hidden.window().hide().unwrap();
        let _ = commands.try_send(flowtorrent::app::commands::AppCommand::SetVisible(false));
    });
    #[cfg(target_os = "macos")]
    if relief {
        slint::Timer::single_shot(Duration::from_secs(7), move || {
            unsafe extern "C" {
                fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
            }
            // SAFETY: Apple's public API accepts NULL for all zones and zero for
            // maximal reclamation of unused allocations. No live object is freed.
            let released = unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) };
            println!("allocator released={released}");
            report("after pressure relief");
        });
    }
    #[cfg(not(target_os = "macos"))]
    anyhow::ensure!(!relief, "Allocator experiment supports only macOS");
    slint::Timer::single_shot(Duration::from_secs(10), move || {
        report("hidden");
        slint::quit_event_loop().unwrap();
        drop(ui);
    });
    slint::run_event_loop_until_quit()?;
    Ok(())
}
