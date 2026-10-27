use crate::error::{AppError, AppResult};
use std::path::Path;

fn conflict(path: &Path) -> AppError {
    AppError::FileConflict(path.into())
}
pub fn launch_agent(executable: &Path) -> AppResult<String> {
    let text = executable.to_str().ok_or_else(|| conflict(executable))?;
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;");
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- FlowTorrent login entry -->\n<plist version=\"1.0\"><dict><key>Label</key><string>com.engus.flowtorrent</string><key>ProgramArguments</key><array><string>{escaped}</string></array><key>RunAtLoad</key><true/></dict></plist>\n"
    ))
}
/// Used for a dedicated launch entry only. Refuse foreign files and links.
pub fn write_entry(path: &Path, contents: &str, enabled: bool) -> AppResult<()> {
    use std::io::Write;
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if !metadata.is_file()
            || metadata.is_symlink()
            || std::fs::read_to_string(path)? != contents
        {
            return Err(conflict(path));
        }
    } else if path.try_exists()? {
        return Err(conflict(path));
    }
    if enabled {
        let parent = path.parent().ok_or_else(|| conflict(path))?;
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(contents.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|e| e.error)?;
    } else {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
pub trait LoginChange: Send {
    fn rollback(self: Box<Self>) -> AppResult<()>;
}
impl<F: FnOnce() -> AppResult<()> + Send> LoginChange for F {
    fn rollback(self: Box<Self>) -> AppResult<()> {
        (*self)()
    }
}
pub trait LoginProvider: Send + Sync {
    fn apply(&self, enabled: bool) -> AppResult<Box<dyn LoginChange>>;
}
pub struct NativeLogin;
impl LoginProvider for NativeLogin {
    fn apply(&self, enabled: bool) -> AppResult<Box<dyn LoginChange>> {
        native_change(enabled).map_err(|e| AppError::LoginUnavailable(e.to_string()))
    }
}
fn native_change(enabled: bool) -> AppResult<Box<dyn LoginChange>> {
    let executable = std::env::current_exe()?;
    #[cfg(target_os = "macos")]
    {
        if !executable
            .ancestors()
            .any(|p| p.extension().is_some_and(|s| s == "app"))
        {
            return Err(AppError::InvalidMetadata(
                "Launch at login requires an installed .app bundle".into(),
            ));
        }
        let home = directories::UserDirs::new().ok_or(AppError::MissingSystemFolder)?;
        let path = home
            .home_dir()
            .join("Library/LaunchAgents/com.engus.flowtorrent.plist");
        let contents = launch_agent(&executable)?;
        let previous = path.try_exists()?;
        write_entry(&path, &contents, enabled)?;
        Ok(Box::new(move || write_entry(&path, &contents, previous)))
    }
    #[cfg(windows)]
    {
        let previous = windows_entry(&executable, enabled)?;
        Ok(Box::new(move || {
            windows_entry(&executable, previous).map(|_| ())
        }))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (enabled, executable);
        Err(AppError::InvalidMetadata(
            "Launch at login is unsupported on this platform".into(),
        ))
    }
}
#[cfg(windows)]
fn windows_entry(executable: &Path, enabled: bool) -> AppResult<bool> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::*,
    };
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let key_name = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let value_name = wide("FlowTorrent.com.engus.flowtorrent");
    let mut command = vec![b'"' as u16];
    command.extend(executable.as_os_str().encode_wide());
    command.extend([b'"' as u16, 0]);
    let mut key = std::ptr::null_mut();
    // SAFETY: all UTF-16 inputs are terminated and output key points to live storage.
    let result = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            key_name.as_ptr(),
            0,
            std::ptr::null(),
            0,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    if result != ERROR_SUCCESS {
        return Err(std::io::Error::from_raw_os_error(result as i32).into());
    }
    struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: owns the key returned by RegCreateKeyExW.
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }
    let key = Key(key);
    let mut existing = vec![0u16; 32768];
    let mut size = (existing.len() * 2) as u32;
    // SAFETY: allocated output has size bytes, key is live, value name terminated.
    let read = unsafe {
        RegGetValueW(
            key.0,
            std::ptr::null(),
            value_name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            existing.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if read == ERROR_SUCCESS {
        existing.truncate(size as usize / 2);
        if existing != command {
            return Err(conflict(executable));
        }
    } else if read != ERROR_FILE_NOT_FOUND {
        return Err(std::io::Error::from_raw_os_error(read as i32).into());
    }
    // SAFETY: input bytes remain live through the synchronous call; owned key/name are valid.
    let result = unsafe {
        if enabled {
            RegSetValueExW(
                key.0,
                value_name.as_ptr(),
                0,
                REG_SZ,
                command.as_ptr().cast(),
                (command.len() * 2) as u32,
            )
        } else {
            RegDeleteValueW(key.0, value_name.as_ptr())
        }
    };
    if result == ERROR_SUCCESS || !enabled && result == ERROR_FILE_NOT_FOUND {
        Ok(read == ERROR_SUCCESS)
    } else {
        Err(std::io::Error::from_raw_os_error(result as i32).into())
    }
}
