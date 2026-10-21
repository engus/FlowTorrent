//! Extend the existing Winit delegate, preserving its identity, ivars and methods.
use super::PlatformEvent;
use crate::error::AppResult;
use objc2::{
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel},
    sel,
};
use objc2_app_kit::{NSApplication, NSApplicationDelegateReply, NSApplicationTerminateReply};
use objc2_foundation::{MainThreadMarker, NSArray, NSString};
use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
};

static EVENTS: Mutex<Option<Sender<PlatformEvent>>> = Mutex::new(None);
static TERMINATION_PENDING: AtomicBool = AtomicBool::new(false);
pub struct MacHooks {
    delegate: Retained<AnyObject>,
    original: &'static AnyClass,
}
impl MacHooks {
    pub fn attach(events: Sender<PlatformEvent>) -> AppResult<Self> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| anyhow::anyhow!("AppKit integration requires the main thread"))?;
        let app = NSApplication::sharedApplication(mtm);
        let delegate = app
            .delegate()
            .ok_or_else(|| anyhow::anyhow!("Winit application delegate is unavailable"))?;
        // SAFETY: A protocol object is an Objective-C object. Erase only its
        // protocol wrapper; inspect the actual runtime class before extending it.
        let delegate: Retained<AnyObject> = unsafe { Retained::cast_unchecked(delegate) };
        let original = delegate.class();
        if original.name().to_bytes() != b"WinitApplicationDelegate" {
            return Err(anyhow::anyhow!(
                "Unexpected application delegate: {}",
                original.name().to_string_lossy()
            )
            .into());
        }
        let class = if let Some(class) = AnyClass::get(c"FlowTorrentWinitDelegate") {
            class
        } else {
            for selector in [
                sel!(application:openFiles:),
                sel!(application:openFile:),
                sel!(applicationShouldHandleReopen:hasVisibleWindows:),
                sel!(applicationShouldTerminate:),
                sel!(applicationShouldTerminateAfterLastWindowClosed:),
            ] {
                if original.instance_method(selector).is_some() {
                    return Err(anyhow::anyhow!(
                        "Winit now handles {selector}; the native adapter needs review"
                    )
                    .into());
                }
            }
            let mut builder = ClassBuilder::new(c"FlowTorrentWinitDelegate", original)
                .ok_or_else(|| anyhow::anyhow!("Cannot register native delegate extension"))?;
            // SAFETY: These signatures exactly match NSApplicationDelegate. No
            // inherited method/ivar is replaced or added; Winit's isKindOf check,
            // launch/termination callbacks and Rust ivar layout remain valid.
            unsafe {
                builder.add_method(
                    sel!(application:openFiles:),
                    open_files
                        as extern "C" fn(&AnyObject, Sel, &NSApplication, &NSArray<NSString>),
                );
                builder.add_method(
                    sel!(application:openFile:),
                    open_file as extern "C" fn(&AnyObject, Sel, &NSApplication, &NSString) -> Bool,
                );
                builder.add_method(
                    sel!(applicationShouldHandleReopen:hasVisibleWindows:),
                    reopen as extern "C" fn(&AnyObject, Sel, &NSApplication, Bool) -> Bool,
                );
                builder.add_method(
                    sel!(applicationShouldTerminate:),
                    terminate
                        as extern "C" fn(
                            &AnyObject,
                            Sel,
                            &NSApplication,
                        ) -> NSApplicationTerminateReply,
                );
                builder.add_method(
                    sel!(applicationShouldTerminateAfterLastWindowClosed:),
                    close_last as extern "C" fn(&AnyObject, Sel, &NSApplication) -> Bool,
                );
            }
            builder.register()
        };
        // SAFETY: Main thread; subclass adds no ivars and inherits every existing
        // implementation. Its instance size and destruction layout are identical.
        unsafe {
            AnyObject::set_class(&delegate, class);
        }
        *EVENTS.lock().unwrap() = Some(events);
        Ok(Self { delegate, original })
    }
}
impl Drop for MacHooks {
    fn drop(&mut self) {
        *EVENTS.lock().unwrap() = None;
        // SAFETY: Same instance/layout as attach; no callbacks run concurrently
        // because AppKit integration and destruction are on the main thread.
        unsafe {
            AnyObject::set_class(&self.delegate, self.original);
        }
    }
}
fn emit(event: PlatformEvent) -> bool {
    EVENTS
        .lock()
        .ok()
        .and_then(|sender| sender.as_ref().map(|sender| sender.send(event).is_ok()))
        .unwrap_or(false)
}
fn accepted(paths: &[PathBuf]) -> bool {
    paths.len() <= 64
        && paths.iter().all(|path| {
            path.is_absolute()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("torrent"))
        })
        && paths
            .iter()
            .map(|path| path.as_os_str().len())
            .sum::<usize>()
            <= 256 * 1024
}
extern "C" fn open_files(_: &AnyObject, _: Sel, app: &NSApplication, names: &NSArray<NSString>) {
    let success = if names.count() > 64 {
        false
    } else {
        let paths: Vec<_> = (0..names.count())
            .map(|index| PathBuf::from(names.objectAtIndex(index).to_string()))
            .collect();
        accepted(&paths) && emit(PlatformEvent::OpenTorrentPaths(paths))
    };
    app.replyToOpenOrPrint(if success {
        NSApplicationDelegateReply::Success
    } else {
        NSApplicationDelegateReply::Failure
    });
}
extern "C" fn open_file(_: &AnyObject, _: Sel, _: &NSApplication, name: &NSString) -> Bool {
    let paths = vec![PathBuf::from(name.to_string())];
    Bool::new(accepted(&paths) && emit(PlatformEvent::OpenTorrentPaths(paths)))
}
extern "C" fn reopen(_: &AnyObject, _: Sel, _: &NSApplication, _: Bool) -> Bool {
    emit(PlatformEvent::Reopen);
    Bool::NO
}
extern "C" fn terminate(_: &AnyObject, _: Sel, _: &NSApplication) -> NSApplicationTerminateReply {
    TERMINATION_PENDING.store(true, Ordering::Release);
    if emit(PlatformEvent::Quit) {
        NSApplicationTerminateReply::TerminateLater
    } else {
        TERMINATION_PENDING.store(false, Ordering::Release);
        NSApplicationTerminateReply::TerminateCancel
    }
}
extern "C" fn close_last(_: &AnyObject, _: Sel, _: &NSApplication) -> Bool {
    Bool::NO
}
pub fn activate() {
    if let Some(mtm) = MainThreadMarker::new() {
        #[allow(deprecated)]
        NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    }
}
pub fn finish_exit() {
    if TERMINATION_PENDING.swap(false, Ordering::AcqRel)
        && let Some(mtm) = MainThreadMarker::new()
    {
        NSApplication::sharedApplication(mtm).replyToApplicationShouldTerminate(true);
    }
}
/// Used by the macOS CI executable to verify that Winit's delegate remains its
/// original kind and that native file/reopen events reach our channel.
pub fn probe_native_events(paths: &[PathBuf]) -> AppResult<()> {
    let mtm = MainThreadMarker::new().ok_or_else(|| anyhow::anyhow!("Main thread required"))?;
    let app = NSApplication::sharedApplication(mtm);
    let delegate = app
        .delegate()
        .ok_or_else(|| anyhow::anyhow!("No delegate"))?;
    let original = AnyClass::get(c"WinitApplicationDelegate")
        .ok_or_else(|| anyhow::anyhow!("No Winit class"))?;
    let remains_winit: bool = unsafe { msg_send![&*delegate, isKindOfClass: original] };
    if !remains_winit {
        return Err(anyhow::anyhow!("Winit delegate identity lost").into());
    }
    let strings: Vec<_> = paths
        .iter()
        .map(|path| NSString::from_str(&path.to_string_lossy()))
        .collect();
    let names = NSArray::from_retained_slice(&strings);
    let _: () = unsafe { msg_send![&*delegate, application: &*app, openFiles: &*names] };
    let _: bool = unsafe {
        msg_send![&*delegate, applicationShouldHandleReopen: &*app, hasVisibleWindows: false]
    };
    Ok(())
}
