pub mod instance;
pub mod login;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
mod macos_tray;
pub mod notifications;
pub mod state;
pub mod trash;
pub const APP_ID: &str = "com.engus.flowtorrent";
#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(not(target_os = "macos"))]
use crate::ui::components::{FlowTray, Translations};
use crate::{
    error::AppResult,
    ui::{i18n::Locale, tray_status::TrayStatus},
};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    sync::mpsc::Sender,
};
#[cfg(target_os = "macos")]
type PlatformTray = macos_tray::MacTray;
#[cfg(not(target_os = "macos"))]
type PlatformTray = FlowTray;

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
    tray: RefCell<Option<PlatformTray>>,
    locale: Cell<Locale>,
    status: RefCell<TrayStatus>,
    exiting: Cell<bool>,
    events: Sender<PlatformEvent>,
    #[cfg(target_os = "macos")]
    _mac: macos::MacHooks,
}
impl PlatformShell {
    pub fn set_locale(&self, locale: Locale) {
        if self.locale.replace(locale) == locale {
            return;
        }
        if let Some(tray) = self.tray.borrow().as_ref() {
            apply_locale(tray, locale);
        }
    }
    pub fn set_status(&self, status: &TrayStatus) {
        if *self.status.borrow() == *status {
            return;
        }
        if let Some(tray) = self.tray.borrow().as_ref() {
            apply_status(tray, status);
        }
        *self.status.borrow_mut() = status.clone();
    }
    pub fn attach(_window: &slint::Window, events: Sender<PlatformEvent>) -> AppResult<Self> {
        let locale = Locale::from_system_tag(&sys_locale::get_locale().unwrap_or_default());
        #[cfg(target_os = "macos")]
        let mac = macos::MacHooks::attach(events.clone())?;
        Ok(Self {
            tray: RefCell::new(None),
            locale: Cell::new(locale),
            status: RefCell::new(TrayStatus::from_rows(&[], locale)),
            exiting: Cell::new(false),
            events,
            #[cfg(target_os = "macos")]
            _mac: mac,
        })
    }
    /// Construct only after reading durable intent; disabled startup never flashes an icon.
    pub fn set_tray_enabled(&self, enabled: bool) -> AppResult<()> {
        if self.exiting.get() {
            return Ok(());
        }
        if enabled {
            if let Some(tray) = self.tray.borrow().as_ref() {
                show_tray(tray)?;
                return Ok(());
            }
            let tray = create_tray(
                self.events.clone(),
                self.locale.get(),
                &self.status.borrow(),
            )?;
            *self.tray.borrow_mut() = Some(tray);
        } else {
            let old = self.tray.borrow_mut().take();
            if let Some(tray) = old {
                hide_tray(&tray)?;
            }
        }
        Ok(())
    }
    pub fn has_tray_icon(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.tray
                .borrow()
                .as_ref()
                .is_some_and(PlatformTray::is_visible)
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.tray.borrow().is_some()
        }
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
        self.set_tray_enabled(false)?;
        self.exiting.set(true);
        #[cfg(target_os = "macos")]
        macos::finish_exit();
        Ok(())
    }
}
#[cfg(target_os = "macos")]
fn create_tray(
    events: Sender<PlatformEvent>,
    locale: Locale,
    status: &TrayStatus,
) -> AppResult<PlatformTray> {
    let tray = PlatformTray::new(events, locale, status)?;
    tray.set_status(status);
    Ok(tray)
}
#[cfg(not(target_os = "macos"))]
fn create_tray(
    events: Sender<PlatformEvent>,
    locale: Locale,
    status: &TrayStatus,
) -> AppResult<PlatformTray> {
    let tray = FlowTray::new().map_err(anyhow::Error::from)?;
    tray.global::<Translations>().on_lookup(|index, ru, en| {
        Locale::from_index(index)
            .unwrap_or(Locale::En)
            .text(&ru, &en)
            .into()
    });
    apply_locale(&tray, locale);
    let show = events.clone();
    tray.on_reopen(move || {
        let _ = show.send(PlatformEvent::Reopen);
    });
    let open = events.clone();
    tray.on_open_torrent(move || {
        let _ = open.send(PlatformEvent::OpenDialog);
    });
    tray.on_quit_requested(move || {
        let _ = events.send(PlatformEvent::Quit);
    });
    apply_status(&tray, status);
    tray.show().map_err(anyhow::Error::from)?;
    Ok(tray)
}
fn apply_locale(tray: &PlatformTray, locale: Locale) {
    #[cfg(target_os = "macos")]
    tray.set_locale(locale);
    #[cfg(not(target_os = "macos"))]
    {
        tray.set_ru(locale == Locale::Ru);
        tray.global::<Translations>().set_locale(locale.index());
    }
}
fn apply_status(tray: &PlatformTray, status: &TrayStatus) {
    #[cfg(target_os = "macos")]
    tray.set_status(status);
    #[cfg(not(target_os = "macos"))]
    {
        if tray.get_download_status() != status.download {
            tray.set_download_status(status.download.as_str().into());
        }
        if tray.get_upload_status() != status.upload {
            tray.set_upload_status(status.upload.as_str().into());
        }
        if tray.get_active_status() != status.active {
            tray.set_active_status(status.active.as_str().into());
        }
        if tray.get_status_tooltip() != status.tooltip {
            tray.set_status_tooltip(status.tooltip.as_str().into());
        }
    }
}
fn show_tray(tray: &PlatformTray) -> AppResult<()> {
    #[cfg(target_os = "macos")]
    tray.show();
    #[cfg(not(target_os = "macos"))]
    tray.show().map_err(anyhow::Error::from)?;
    Ok(())
}
fn hide_tray(tray: &PlatformTray) -> AppResult<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = tray;
    } // Dropping the owner removes the native item.
    #[cfg(not(target_os = "macos"))]
    tray.hide().map_err(anyhow::Error::from)?;
    Ok(())
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
