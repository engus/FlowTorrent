//! Slint owns the Windows notification-area icon and Winit owns native focus.
//! Cold launches and subsequent launches share the structured IPC path in main.
pub fn initialize() -> crate::error::AppResult<()> {
    let identity: Vec<_> = super::APP_ID.encode_utf16().chain(Some(0)).collect();
    // SAFETY: A valid NUL-terminated UTF-16 application ID, set before any UI.
    let result = unsafe {
        windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(identity.as_ptr())
    };
    if result < 0 {
        return Err(
            anyhow::anyhow!("Cannot set the notification application ID: {result:#x}").into(),
        );
    }
    Ok(())
}
