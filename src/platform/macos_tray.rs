//! A stable native status item, independent of Slint windows and menu rebuilds.
use super::PlatformEvent;
use crate::{
    error::AppResult,
    ui::{i18n::Locale, tray_status::TrayStatus},
};
use objc2::{
    AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained, sel,
};
use objc2_app_kit::{
    NSBitmapFormat, NSBitmapImageRep, NSCellImagePosition, NSDeviceRGBColorSpace, NSImage, NSMenu,
    NSMenuItem, NSStatusBar, NSStatusBarButton, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSSize, NSString};
use std::sync::mpsc::Sender;

struct ActionIvars {
    events: Sender<PlatformEvent>,
}
define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ActionIvars]
    struct FlowTorrentStatusActions;
    // SAFETY: This NSObject subclass adds only Rust-owned ivars and main-thread actions.
    unsafe impl NSObjectProtocol for FlowTorrentStatusActions {}
    impl FlowTorrentStatusActions {
        #[unsafe(method(activateItem:))]
        fn activate_item(&self, item: &NSMenuItem) {
            let event = match item.tag() {
                1 => PlatformEvent::Reopen,
                2 => PlatformEvent::OpenDialog,
                3 => PlatformEvent::Quit,
                _ => return,
            };
            let _ = self.ivars().events.send(event);
        }
    }
);
impl FlowTorrentStatusActions {
    fn new(mtm: MainThreadMarker, events: Sender<PlatformEvent>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ActionIvars { events });
        // SAFETY: Initialize the freshly allocated NSObject subclass on its main thread.
        unsafe { msg_send![super(this), init] }
    }
}

pub(super) struct MacTray {
    item: Retained<NSStatusItem>,
    button: Retained<NSStatusBarButton>,
    _actions: Retained<FlowTorrentStatusActions>,
    _menu: Retained<NSMenu>,
    status: Retained<NSMenuItem>,
    commands: [Retained<NSMenuItem>; 3],
}
impl MacTray {
    pub fn new(
        events: Sender<PlatformEvent>,
        locale: Locale,
        status: &TrayStatus,
    ) -> AppResult<Self> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| anyhow::anyhow!("Status item requires the main thread"))?;
        let image = status_image()?;
        let actions = FlowTorrentStatusActions::new(mtm, events);
        let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("FlowTorrent"));
        menu.setAutoenablesItems(false);
        let status = menu_item(mtm, &status.summary, None);
        status.setEnabled(false);
        menu.addItem(&status);
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        let commands = ["", "", ""].map(|text| menu_item(mtm, text, Some(sel!(activateItem:))));
        for (index, item) in commands.iter().enumerate() {
            if index == 2 {
                menu.addItem(&NSMenuItem::separatorItem(mtm));
            }
            item.setTag(index as isize + 1);
            // SAFETY: The action target is strongly retained by MacTray until the
            // native item is detached; the selector has the declared sender signature.
            unsafe {
                item.setTarget(Some(actions.as_ref()));
            }
            menu.addItem(item);
        }
        let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        let Some(button) = item.button(mtm) else {
            NSStatusBar::systemStatusBar().removeStatusItem(&item);
            return Err(anyhow::anyhow!("Status bar button is unavailable").into());
        };
        button.setImage(Some(&image));
        button.setImagePosition(NSCellImagePosition::ImageOnly);
        item.setMenu(Some(&menu));
        item.setVisible(true);
        let tray = Self {
            item,
            button,
            _actions: actions,
            _menu: menu,
            status,
            commands,
        };
        tray.set_locale(locale);
        Ok(tray)
    }
    pub fn is_visible(&self) -> bool {
        self.item.isVisible()
    }
    pub fn show(&self) {
        if !self.item.isVisible() {
            self.item.setVisible(true);
        }
    }
    pub fn set_locale(&self, locale: Locale) {
        for (item, text) in self.commands.iter().zip([
            locale.text("Открыть FlowTorrent", "Show FlowTorrent"),
            locale.text("Добавить торрент…", "Add torrent…"),
            locale.text("Выйти", "Quit"),
        ]) {
            item.setTitle(&NSString::from_str(text));
        }
    }
    pub fn set_status(&self, status: &TrayStatus) {
        let value = NSString::from_str(&status.summary);
        if *self.status.title() != *value {
            self.status.setTitle(&value);
        }
        self.button
            .setToolTip(Some(&NSString::from_str(&status.tooltip)));
    }
}
impl Drop for MacTray {
    fn drop(&mut self) {
        self.item.setMenu(None);
        NSStatusBar::systemStatusBar().removeStatusItem(&self.item);
    }
}
fn menu_item(
    mtm: MainThreadMarker,
    title: &str,
    action: Option<objc2::runtime::Sel>,
) -> Retained<NSMenuItem> {
    // SAFETY: Each nonempty action is activateItem: on the retained target above;
    // status items have no selector. No keyboard equivalent is installed globally.
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            action,
            &NSString::new(),
        )
    }
}
fn status_image() -> anyhow::Result<Retained<NSImage>> {
    let image =
        slint::Image::load_from_svg_data(include_bytes!("../../assets/brand/status-icon.svg"))?;
    let pixels = image
        .to_rgba8()
        .ok_or_else(|| anyhow::anyhow!("Cannot rasterize status icon"))?;
    anyhow::ensure!(
        pixels.width() == 36 && pixels.height() == 36,
        "Unexpected status icon dimensions"
    );
    // SAFETY: A null planes pointer requests AppKit-owned storage. The explicit
    // interleaved RGBA8, non-premultiplied format matches Slint's to_rgba8 buffer.
    let bitmap = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(), std::ptr::null_mut(), 36, 36, 8, 4, true, false,
            NSDeviceRGBColorSpace, NSBitmapFormat::AlphaNonpremultiplied, 36 * 4, 32,
        )
    }.ok_or_else(|| anyhow::anyhow!("Cannot allocate status icon bitmap"))?;
    let data = bitmap.bitmapData();
    anyhow::ensure!(!data.is_null(), "Status icon bitmap has no storage");
    // SAFETY: AppKit allocated 36 rows of 36*4 bytes, and pixels is exactly that
    // size. These distinct owned allocations cannot overlap; both remain live.
    unsafe {
        std::ptr::copy_nonoverlapping(pixels.as_bytes().as_ptr(), data, pixels.as_bytes().len());
    }
    let result = NSImage::initWithSize(NSImage::alloc(), NSSize::new(18.0, 18.0));
    result.addRepresentation(&bitmap);
    result.setTemplate(true);
    Ok(result)
}
