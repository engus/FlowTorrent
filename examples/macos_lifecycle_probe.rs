//! CI AppKit/Winit integration probe; real Finder/Dock interaction remains manual.
#[cfg(not(target_os = "macos"))]
fn main() {}
#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    use flowtorrent::{
        platform::{PlatformEvent, PlatformShell, macos},
        ui::components::MainWindow,
    };
    use slint::ComponentHandle;
    use std::{
        rc::Rc,
        time::{Duration, Instant},
    };
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let window = MainWindow::new()?;
    window.show()?;
    let (events, receiver) = std::sync::mpsc::channel();
    let shell = Rc::new(PlatformShell::attach(window.window(), events)?);
    assert!(
        !shell.has_tray_icon(),
        "tray was created before reading saved preference"
    );
    shell.set_tray_enabled(true)?;
    assert!(shell.has_tray_icon());

    let paths = vec![
        std::path::PathBuf::from("/tmp/Flow Torrent.torrent"),
        std::path::PathBuf::from("/tmp/кириллица.torrent"),
    ];
    macos::probe_native_events(&paths)?;
    assert!(
        matches!(receiver.recv_timeout(Duration::from_secs(2))?, PlatformEvent::OpenTorrentPaths(received) if received == paths)
    );
    assert!(matches!(
        receiver.recv_timeout(Duration::from_secs(2))?,
        PlatformEvent::Reopen
    ));
    window.hide()?;
    assert!(
        shell.has_tray_icon(),
        "hiding the window dropped its independent tray"
    );
    shell.set_tray_enabled(false)?;
    assert!(!shell.has_tray_icon());
    shell.set_tray_enabled(true)?;
    assert!(shell.has_tray_icon());
    let started = Instant::now();
    let weak = window.as_weak();
    slint::Timer::single_shot(Duration::from_millis(500), move || {
        if let Some(window) = weak.upgrade() {
            window.show().unwrap();
            window.hide().unwrap();
        }
        shell.finish_exit().unwrap();
        shell.set_tray_enabled(true).unwrap();
        assert!(
            !shell.has_tray_icon(),
            "late snapshot recreated the tray during exit"
        );
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop_until_quit()?;
    anyhow::ensure!(
        started.elapsed() >= Duration::from_millis(400),
        "Event loop exited while hidden"
    );
    println!(
        "Winit delegate identity, ordered AppKit file callbacks, reopen callback and hidden event loop: PASS"
    );
    Ok(())
}
