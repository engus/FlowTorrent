//! Native document modality; no retained Winit handles or nested event loop.
use crate::error::AppResult;
use slint::winit_030::{
    WinitWindowAccessor,
    winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
};

pub struct ModalGuard {
    inner: native::Session,
}
impl ModalGuard {
    pub fn ready(parent: &slint::Window, child: &slint::Window) -> bool {
        parent.has_winit_window() && child.has_winit_window()
    }
    pub fn attach(parent: &slint::Window, child: &slint::Window) -> AppResult<Self> {
        let raw = |window: &slint::Window| -> AppResult<RawWindowHandle> {
            Ok(window
                .with_winit_window(|w| w.window_handle().map(|h| h.as_raw()))
                .ok_or_else(|| anyhow::anyhow!("Native window is not ready"))?
                .map_err(anyhow::Error::from)?)
        };
        Ok(Self {
            inner: native::Session::attach(raw(parent)?, raw(child)?)?,
        })
    }
    pub fn is_attached(&self) -> bool {
        self.inner.is_attached()
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use objc2::{MainThreadMarker, rc::Retained};
    use objc2_app_kit::{NSView, NSWindow};
    pub struct Session {
        parent: Retained<NSWindow>,
        child: Retained<NSWindow>,
    }
    impl Session {
        pub fn attach(parent: RawWindowHandle, child: RawWindowHandle) -> AppResult<Self> {
            if MainThreadMarker::new().is_none() {
                return Err(anyhow::anyhow!("Modal ownership requires the main thread").into());
            }
            let window = |raw| -> AppResult<Retained<NSWindow>> {
                let RawWindowHandle::AppKit(raw) = raw else {
                    return Err(anyhow::anyhow!("Expected an AppKit window").into());
                };
                // SAFETY: Winit supplied a live NSView; both Slint components are
                // borrowed on their UI thread throughout attachment.
                let view = unsafe { raw.ns_view.cast::<NSView>().as_ref() };
                view.window()
                    .ok_or_else(|| anyhow::anyhow!("Native view has no window").into())
            };
            let parent = window(parent)?;
            let child = window(child)?;
            if parent.attachedSheet().is_some() || child.sheetParent().is_some() {
                return Err(anyhow::anyhow!("A modal sheet is already active").into());
            }
            child.orderOut(None);
            parent.beginSheet_completionHandler(&child, None);
            Ok(Self { parent, child })
        }
        pub fn is_attached(&self) -> bool {
            self.child
                .sheetParent()
                .is_some_and(|p| std::ptr::eq::<NSWindow>(p.as_ref(), self.parent.as_ref()))
        }
    }
    impl Drop for Session {
        fn drop(&mut self) {
            if self.is_attached() {
                self.parent.endSheet(&self.child);
            }
            self.child.orderOut(None);
        }
    }
}

#[cfg(target_os = "windows")]
mod native {
    use super::*;
    use windows_sys::Win32::{
        Foundation::{GetLastError, HWND, SetLastError},
        UI::{
            Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
            WindowsAndMessaging::{
                GW_OWNER, GWLP_HWNDPARENT, GetWindow, IsWindow, SetWindowLongPtrW,
            },
        },
    };
    pub struct Session {
        parent: HWND,
        child: HWND,
        owner: isize,
    }
    impl Session {
        pub fn attach(parent: RawWindowHandle, child: RawWindowHandle) -> AppResult<Self> {
            let (RawWindowHandle::Win32(parent), RawWindowHandle::Win32(child)) = (parent, child)
            else {
                return Err(anyhow::anyhow!("Expected Win32 windows").into());
            };
            let parent = parent.hwnd.get() as HWND;
            let child = child.hwnd.get() as HWND;
            // SAFETY: These are live, same-thread Winit windows, and the UI
            // coordinator releases this lease before hiding either component.
            unsafe {
                if IsWindowEnabled(parent) == 0 {
                    return Err(anyhow::anyhow!("The owner already has a modal window").into());
                }
                SetLastError(0);
                let owner = SetWindowLongPtrW(child, GWLP_HWNDPARENT, parent as isize);
                if owner == 0 && GetLastError() != 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
                EnableWindow(parent, 0);
                Ok(Self {
                    parent,
                    child,
                    owner,
                })
            }
        }
        pub fn is_attached(&self) -> bool {
            // SAFETY: The coordinator retains both components until lease drop.
            unsafe {
                IsWindow(self.child) != 0
                    && GetWindow(self.child, GW_OWNER) == self.parent
                    && IsWindowEnabled(self.parent) == 0
            }
        }
    }
    impl Drop for Session {
        fn drop(&mut self) {
            // SAFETY: Restore only still-live owned windows on the UI thread.
            unsafe {
                if IsWindow(self.child) != 0 {
                    SetWindowLongPtrW(self.child, GWLP_HWNDPARENT, self.owner);
                }
                if IsWindow(self.parent) != 0 {
                    EnableWindow(self.parent, 1);
                }
            }
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod native {
    use super::*;
    pub struct Session;
    impl Session {
        pub fn attach(_: RawWindowHandle, _: RawWindowHandle) -> AppResult<Self> {
            Err(anyhow::anyhow!("Native modal windows require macOS or Windows").into())
        }
        pub fn is_attached(&self) -> bool {
            false
        }
    }
}
