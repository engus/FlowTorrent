#[path = "../../tests/support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
mod memory;
#[path = "../../tests/support/mod.rs"]
#[allow(dead_code)]
mod support;
use anyhow::{Context, Result, ensure};
use flowtorrent::{
    app::{AppController, commands::AppCommand, storage::SessionStore},
    domain::*,
    engine::librqbit::LibrqbitEngine,
    platform::{PlatformEvent, PlatformShell},
    settings::{Settings, SettingsStore},
    ui::{UiController, i18n::Locale},
};
use std::{
    ffi::OsString,
    fs::File,
    io::Write,
    num::NonZeroU32,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

struct Options {
    root: PathBuf,
    scenario: String,
    seconds: u64,
}
impl Options {
    fn parse(args: &[OsString]) -> Result<Self> {
        ensure!(
            args.len() == 3,
            "Usage: measure_resources <new-absolute-directory> <empty|active5|paused20|hidden5|cycles5> <seconds:1..86400>"
        );
        let root = PathBuf::from(&args[0]);
        ensure!(root.is_absolute(), "Directory must be absolute");
        let scenario = args[1].to_str().context("scenario")?.to_owned();
        ensure!(
            ["empty", "active5", "paused20", "hidden5", "cycles5"].contains(&scenario.as_str()),
            "Unknown scenario"
        );
        let seconds: u64 = args[2].to_str().context("duration")?.parse()?;
        ensure!(
            (1..=86400).contains(&seconds),
            "Duration must be 1..86400 seconds"
        );
        Ok(Self {
            root,
            scenario,
            seconds,
        })
    }
    fn create_root(&self) -> Result<()> {
        std::fs::create_dir(&self.root)
            .context("Use a NEW fixture directory; existing files are never adopted")
    }
    fn active(&self) -> bool {
        self.scenario.ends_with('5')
    }
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
fn spawn_child(root: &Path, role: &str, scenario: &str) -> Result<OwnedChild> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args([
            OsString::from(role),
            root.as_os_str().into(),
            scenario.into(),
        ])
        .stdin(Stdio::null())
        .stdout(File::create(root.join(format!("{role}.stdout.log")))?)
        .stderr(File::create(root.join(format!("{role}.stderr.log")))?);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW: helpers have no console.
    }
    Ok(OwnedChild(command.spawn()?))
}
fn wait_ready(child: &mut OwnedChild, file: &Path) -> Result<()> {
    let start = Instant::now();
    while !file.exists() {
        ensure!(
            child.0.try_wait()?.is_none(),
            "Child exited before readiness; see fixture logs"
        );
        ensure!(
            start.elapsed() < Duration::from_secs(180),
            "Child readiness timeout; see fixture logs"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
fn finish(child: &mut OwnedChild, marker: &Path) -> Result<()> {
    std::fs::write(marker, b"stop")?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.0.try_wait()? {
            ensure!(status.success(), "Child failed: {status}");
            return Ok(());
        }
        ensure!(
            start.elapsed() < Duration::from_secs(30),
            "Child graceful shutdown timed out"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}
pub fn run(args: Vec<OsString>) -> Result<()> {
    if args
        .first()
        .is_some_and(|arg| arg == "--seeder" || arg == "--client")
    {
        ensure!(args.len() == 3, "Internal child arguments");
        let root = PathBuf::from(&args[1]);
        ensure!(
            root.is_absolute() && root.join("probe.fixture").is_file(),
            "Missing owned probe marker"
        );
        return if args[0] == "--seeder" {
            seeder(&root)
        } else {
            client(root, args[2].to_str().context("scenario")?.into())
        };
    }
    let options = Options::parse(&args)?;
    ensure!(
        !cfg!(debug_assertions),
        "Build with --release for resource measurements"
    );
    options.create_root()?;
    let root = options.root.canonicalize()?;
    std::fs::write(root.join("probe.fixture"), b"FlowTorrent resource probe")?;
    let mut seed = if options.active() {
        let mut child = spawn_child(&root, "--seeder", &options.scenario)?;
        wait_ready(&mut child, &root.join("seed.ready"))?;
        Some(child)
    } else {
        None
    };
    let mut app = spawn_child(&root, "--client", &options.scenario)?;
    wait_ready(&mut app, &root.join("client.ready"))?;
    wait_ready(&mut app, &root.join("client.status"))?;
    let os_version = if cfg!(windows) {
        Command::new("cmd").args(["/C", "ver"]).output()
    } else {
        Command::new("sw_vers").output()
    }
    .ok()
    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    };
    let display: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("client.ready"))?)?;
    let environment = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"), "os": std::env::consts::OS, "os_version": os_version,
        "arch": std::env::consts::ARCH, "profile": "release", "renderer": "software",
        "scenario": options.scenario, "duration_seconds": options.seconds, "warmup_seconds": 20,
        "client_pid": app.0.id(), "seeder_pid": seed.as_ref().map(|s| s.0.id()),
        "revision_in_working_directory": git(&["rev-parse", "HEAD"]),
        "working_directory_status": git(&["status", "--porcelain"]),
        "active_file_bytes": 67_108_864, "paused_file_bytes": 131_072,
        "global_download_limit_bps": if options.active() { Some(655_360) } else { None },
        "public_discovery": false, "measured_process": "client only", "display": display
    });
    std::fs::write(
        root.join("environment.json"),
        serde_json::to_vec_pretty(&environment)?,
    )?;
    for _ in 0..20 {
        ensure!(app.0.try_wait()?.is_none(), "Client exited during warmup");
        std::thread::sleep(Duration::from_secs(1));
    }
    let mut output = File::create(root.join("samples.csv"))?;
    writeln!(
        output,
        "elapsed_seconds,resident_bytes,private_bytes,physical_footprint_bytes,cpu_time_ns,checked_bytes,visible"
    )?;
    let start = Instant::now();
    for _ in 0..options.seconds {
        ensure!(
            app.0.try_wait()?.is_none(),
            "Client exited during measurement"
        );
        if let Some(seed) = seed.as_mut() {
            ensure!(
                seed.0.try_wait()?.is_none(),
                "Seeder exited during measurement"
            );
        }
        let sample = memory::sample(app.0.id())?;
        let status: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("client.status"))?)?;
        writeln!(
            output,
            "{:.3},{},{},{},{},{},{}",
            start.elapsed().as_secs_f64(),
            sample.resident_bytes,
            sample
                .private_bytes
                .map(|n| n.to_string())
                .unwrap_or_default(),
            sample
                .physical_footprint_bytes
                .map(|n| n.to_string())
                .unwrap_or_default(),
            sample.cpu_time_ns,
            status["checked_bytes"],
            status["visible"]
        )?;
        output.flush()?;
        std::thread::sleep(Duration::from_secs(1));
    }
    finish(&mut app, &root.join("client.stop"))?;
    if let Some(seed) = seed.as_mut() {
        finish(seed, &root.join("seed.stop"))?;
    }
    println!("Saved {}", root.join("samples.csv").display());
    Ok(())
}

fn metadata(bytes: &[u8], index: usize) -> Vec<u8> {
    let original = support::torrent_bytes(bytes, false);
    let marker = b"11:fixture.bin";
    let position = original
        .windows(marker.len())
        .position(|p| p == marker)
        .unwrap();
    let mut changed = original[..position].to_vec();
    changed.extend(format!("14:fixture-{index:02}.bin").as_bytes());
    changed.extend_from_slice(&original[position + marker.len()..]);
    changed
}
fn seeder(root: &Path) -> Result<()> {
    use librqbit::{AddTorrent, AddTorrentOptions, Session};
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let source = root.join("seed");
        std::fs::create_dir(&source)?;
        let payload = support::payload().repeat(128);
        let session = Session::new_with_opts(source.clone(), support::local_options()).await?;
        for index in 0..5 {
            let metadata = metadata(&payload, index);
            std::fs::write(source.join(format!("fixture-{index:02}.bin")), &payload)?;
            std::fs::write(root.join(format!("{index}.torrent")), &metadata)?;
            let handle = session
                .add_torrent(
                    AddTorrent::from_bytes(metadata),
                    Some(AddTorrentOptions {
                        overwrite: true,
                        ..Default::default()
                    }),
                )
                .await?
                .into_handle()
                .context("seed handle")?;
            tokio::time::timeout(Duration::from_secs(60), handle.wait_until_completed()).await??;
        }
        std::fs::write(
            root.join("seed.ready"),
            session.listen_addr().context("seed listener")?.to_string(),
        )?;
        while !root.join("seed.stop").exists() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        session.stop().await;
        Ok(())
    })
}

fn client(root: PathBuf, scenario: String) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let active = scenario.ends_with('5');
    let count = if active {
        5
    } else if scenario == "paused20" {
        20
    } else {
        0
    };
    let paths = lifecycle::paths(&root);
    std::fs::create_dir_all(&paths.download_dir)?;
    let handle = runtime.block_on(async {
        let settings = Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        ));
        let mut defaults = Settings::defaults(paths.download_dir.clone());
        defaults.notify_on_complete = false;
        defaults.limits.download_bps = if active {
            NonZeroU32::new(655_360)
        } else {
            None
        };
        settings.save(&defaults)?;
        let peers = if active {
            vec![std::fs::read_to_string(root.join("seed.ready"))?.parse()?]
        } else {
            vec![]
        };
        let engine = LibrqbitEngine::with_options(
            paths.download_dir.clone(),
            support::local_options(),
            peers,
        )
        .await?;
        let mut handle = AppController::spawn(
            Box::new(engine),
            Arc::new(SessionStore::new(paths.clone())),
            settings,
            paths.clone(),
        );
        for index in 0..count {
            let input = root.join(format!("{index}.torrent"));
            let bytes = if active {
                std::fs::read(&input)?
            } else {
                metadata(&support::payload()[..131_072], index)
            };
            let id = lifecycle::add(&mut handle, &input, &paths.download_dir, &bytes, None).await?;
            if !active {
                handle.commands.send(AppCommand::Pause(id)).await?;
            }
        }
        lifecycle::until(&mut handle, |s| {
            s.torrents.len() == count
                && s.torrents.iter().all(|r| {
                    if active {
                        r.state == TransferState::Downloading
                    } else {
                        r.state == TransferState::Paused
                    }
                })
        })
        .await?;
        Ok::<_, anyhow::Error>(handle)
    })?;
    let commands = handle.commands.clone();
    let observer = handle.snapshots.clone();
    let status = root.join("client.status");
    runtime.spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let snapshot = observer.borrow().clone();
            let checked: u64 = snapshot
                .torrents
                .iter()
                .map(|r| r.checked_downloaded_bytes)
                .sum();
            // Atomic replacement avoids readers seeing half a CSV status pair.
            let value = serde_json::json!({"checked_bytes": checked, "visible": snapshot.visible});
            let _ = flowtorrent::persistence::atomic_write_json(&status, &value);
            if observer.has_changed().is_err() {
                break;
            }
        }
    });
    let (events, receiver) = std::sync::mpsc::channel();
    let _entered = runtime.enter();
    let ui = Rc::new(UiController::new_with_locale(handle, Locale::En)?);
    let shell = Rc::new(PlatformShell::attach(ui.window(), events.clone())?);
    ui.enable_background(shell, receiver, root.join("state/shell.json"))?;
    ui.window().show()?;
    if scenario == "hidden5" {
        ui.window().hide()?;
        commands.try_send(AppCommand::SetVisible(false))?;
    }
    let stop_marker = root.join("client.stop");
    let quit_events = events.clone();
    runtime.spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if stop_marker.exists() {
                let _ = quit_events.send(PlatformEvent::Quit);
                break;
            }
        }
    });
    let timer = slint::Timer::default();
    let ready = root.join("client.ready");
    std::fs::write(
        ready,
        serde_json::to_vec(&serde_json::json!({"scale_factor": ui.window().scale_factor()}))?,
    )?;
    if scenario == "cycles5" {
        let weak = Rc::downgrade(&ui);
        let mut visible = true;
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_secs(5),
            move || {
                visible = !visible;
                if let Some(ui) = weak.upgrade() {
                    let _ = if visible {
                        ui.window().show()
                    } else {
                        ui.window().hide()
                    };
                }
                let _ = commands.try_send(AppCommand::SetVisible(visible));
            },
        );
    }
    slint::run_event_loop_until_quit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_workload_and_existing_directory_cannot_start_a_probe() -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        assert!(Options::parse(&[root.path().into(), "invalid".into(), "60".into()]).is_err());
        assert!(Options::parse(&["relative".into(), "empty".into(), "60".into()]).is_err());
        for seconds in ["0", "-1", "oops", "86401"] {
            assert!(Options::parse(&[root.path().into(), "empty".into(), seconds.into()]).is_err());
        }
        let options = Options::parse(&[root.path().into(), "empty".into(), "60".into()])?;
        std::fs::write(root.path().join("sentinel"), b"keep")?;
        assert!(options.create_root().is_err());
        assert_eq!(std::fs::read(root.path().join("sentinel"))?, b"keep");
        Ok(())
    }
    #[test]
    fn sampler_reports_the_owned_process_with_separate_native_metrics() -> anyhow::Result<()> {
        let sample = memory::sample(std::process::id())?;
        assert!(sample.resident_bytes > 0);
        #[cfg(windows)]
        assert!(
            sample.private_bytes.is_some_and(|n| n > 0)
                && sample.physical_footprint_bytes.is_none()
        );
        #[cfg(target_os = "macos")]
        assert!(
            sample.physical_footprint_bytes.is_some_and(|n| n > 0)
                && sample.private_bytes.is_none()
        );
        Ok(())
    }
}
