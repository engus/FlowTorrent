pub mod instance;
pub mod login;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod notifications;
pub mod state;
pub mod trash;
pub const APP_ID: &str = "com.engus.flowtorrent";
#[cfg(target_os = "windows")]
pub mod windows;
use crate::{
    error::AppResult,
    ui::{components::FlowTray, i18n::Locale},
};
use std::{path::PathBuf, sync::mpsc::Sender};

#[derive(Debug)]
pub enum PlatformEvent {
    OpenTorrentPaths(Vec<PathBuf>),
    #[doc(hidden)]
    QueuedOpen {
        paths: Vec<PathBuf>,
        delivered: tokio::sync::oneshot::Sender<()>,
    },
    OpenDialog,
    Reopen,
    Hide,
    Quit,
    ThemeChanged(bool),
}
/// Keep the bounded instance queue bounded through the native/UI bridge as well:
/// only one batch may wait there while core-channel capacity is exhausted.
pub async fn forward_instance_events(
    mut incoming: tokio::sync::mpsc::Receiver<Vec<PathBuf>>,
    events: Sender<PlatformEvent>,
) {
    while let Some(paths) = incoming.recv().await {
        let (delivered, received) = tokio::sync::oneshot::channel();
        if events
            .send(PlatformEvent::QueuedOpen { paths, delivered })
            .is_err()
            || received.await.is_err()
        {
            break;
        }
    }
}
/// Main-thread tray/native hook owner. UI component ownership stays in UiController;
/// events request show/hide instead of retaining a borrowed Window across callbacks.
pub struct PlatformShell {
    tray: FlowTray,
    events: Sender<PlatformEvent>,
    #[cfg(target_os = "macos")]
    _mac: macos::MacHooks,
}
impl PlatformShell {
    pub fn set_locale(&self, locale: Locale) {
        self.tray.set_ru(locale == Locale::Ru);
    }
    pub fn set_status(&self, status: &crate::ui::tray_status::TrayStatus) {
        if self.tray.get_download_status() != status.download {
            self.tray
                .set_download_status(status.download.as_str().into());
        }
        if self.tray.get_upload_status() != status.upload {
            self.tray.set_upload_status(status.upload.as_str().into());
        }
        if self.tray.get_active_status() != status.active {
            self.tray.set_active_status(status.active.as_str().into());
        }
        if self.tray.get_status_tooltip() != status.tooltip {
            self.tray.set_status_tooltip(status.tooltip.as_str().into());
        }
    }
    pub fn attach(_window: &slint::Window, events: Sender<PlatformEvent>) -> AppResult<Self> {
        let tray = FlowTray::new().map_err(anyhow::Error::from)?;
        tray.set_ru(
            Locale::from_system_tag(&sys_locale::get_locale().unwrap_or_default()) == Locale::Ru,
        );
        let show = events.clone();
        tray.on_reopen(move || {
            let _ = show.send(PlatformEvent::Reopen);
        });
        let open = events.clone();
        tray.on_open_torrent(move || {
            let _ = open.send(PlatformEvent::OpenDialog);
        });
        let quit = events.clone();
        tray.on_quit_requested(move || {
            let _ = quit.send(PlatformEvent::Quit);
        });
        #[cfg(target_os = "macos")]
        let mac = macos::MacHooks::attach(events.clone())?;
        let shell = Self {
            tray,
            events,
            #[cfg(target_os = "macos")]
            _mac: mac,
        };
        shell.set_status(&crate::ui::tray_status::TrayStatus::from_rows(
            &[],
            Locale::from_system_tag(&sys_locale::get_locale().unwrap_or_default()),
        ));
        Ok(shell)
    }
    pub fn show_main(&self) -> AppResult<()> {
        self.events
            .send(PlatformEvent::Reopen)
            .map_err(anyhow::Error::from)?;
        Ok(())
    }
    pub fn hide_main(&self) -> AppResult<()> {
        self.events
            .send(PlatformEvent::Hide)
            .map_err(anyhow::Error::from)?;
        Ok(())
    }
    pub fn finish_exit(&self) -> AppResult<()> {
        self.tray.hide().map_err(anyhow::Error::from)?;
        #[cfg(target_os = "macos")]
        macos::finish_exit();
        Ok(())
    }
}
pub fn focus(window: &slint::Window) {
    use slint::winit_030::WinitWindowAccessor;
    window.with_winit_window(|native| {
        native.set_minimized(false);
        native.focus_window();
    });
    #[cfg(target_os = "macos")]
    macos::activate();
}
