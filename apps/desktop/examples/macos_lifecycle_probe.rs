//! CI AppKit/Winit integration probe; real Finder/Dock interaction remains manual.
#[cfg(not(target_os = "macos"))]
fn main() {}
#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    use flowtorrent::{
        platform::{PlatformEvent, PlatformShell, macos},
        ui::components::{AddWindow, MainWindow},
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
    let modal_done = Rc::new(std::cell::Cell::new(false));
    let completed = modal_done.clone();
    let parent = window.as_weak();
    let weak = window.as_weak();
    let exit_shell = shell.clone();
    let started = Instant::now();
    slint::spawn_local(async move {
        use slint::winit_030::WinitWindowAccessor;
        let parent = parent.upgrade().unwrap();
        parent.show().unwrap();
        drop(parent.window().winit_window().await.unwrap());
        let child = AddWindow::new().unwrap();
        for _ in 0..2 {
            child.show().unwrap();
            drop(child.window().winit_window().await.unwrap());
            let lease =
                flowtorrent::platform::modal::ModalGuard::attach(parent.window(), child.window())
                    .unwrap();
            assert!(
                lease.is_attached(),
                "the add window was not a native modal child"
            );
            drop(lease);
            child.hide().unwrap();
        }
        parent.hide().unwrap();
        completed.set(true);
        // Measure the hidden loop after modal setup, independently of runner startup speed.
        slint::Timer::single_shot(Duration::from_millis(1500), move || {
            if let Some(window) = weak.upgrade() {
                window.show().unwrap();
                window.hide().unwrap();
            }
            exit_shell.finish_exit().unwrap();
            exit_shell.set_tray_enabled(true).unwrap();
            assert!(
                !exit_shell.has_tray_icon(),
                "late snapshot recreated the tray during exit"
            );
            slint::quit_event_loop().unwrap();
        });
    })
    .unwrap();
    slint::Timer::single_shot(Duration::from_secs(30), move || {
        assert!(modal_done.get(), "native modal lifecycle did not run");
        panic!("hidden event loop did not finish after native modal setup");
    });
    slint::run_event_loop_until_quit()?;
    anyhow::ensure!(
        started.elapsed() >= Duration::from_millis(400),
        "Event loop exited while hidden"
    );
    println!(
        "Winit delegate identity, ordered AppKit file callbacks, reopen callback, native modal lease and hidden event loop: PASS"
    );
    Ok(())
}
