use crate::error::AppResult;
use std::path::Path;

pub fn move_to_trash(path: &Path) -> AppResult<()> {
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{NSFileManager, NSString, NSURL};
        let name = path
            .to_str()
            .ok_or_else(|| std::io::Error::other("Trash path is not UTF-8"))?;
        let url = NSURL::fileURLWithPath(&NSString::from_str(name));
        let url = url
            .fileReferenceURL()
            .ok_or_else(|| std::io::Error::other("Cannot pin Trash URL"))?;
        NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&url, None)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        Ok(())
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::{
            Win32::{
                System::Com::{
                    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance,
                    CoInitializeEx, CoUninitialize,
                },
                UI::Shell::{
                    FOF_NO_UI, FOFX_EARLYFAILURE, FOFX_RECYCLEONDELETE, FileOperation,
                    IFileOperation, IShellItem, SHCreateItemFromParsingName,
                },
            },
            core::PCWSTR,
        };
        // SAFETY: this synchronous operation owns its COM apartment on the blocking worker.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(anyhow::Error::from)?;
        }
        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                // SAFETY: balances successful CoInitializeEx on this thread.
                unsafe {
                    CoUninitialize();
                }
            }
        }
        let _apartment = Apartment;
        let name = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        // SAFETY: COM is initialized and the path remains terminated/live for the calls.
        unsafe {
            let operation: IFileOperation =
                CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)
                    .map_err(anyhow::Error::from)?;
            operation
                .SetOperationFlags(FOFX_RECYCLEONDELETE | FOFX_EARLYFAILURE | FOF_NO_UI)
                .map_err(anyhow::Error::from)?;
            let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(name.as_ptr()), None)
                .map_err(anyhow::Error::from)?;
            operation
                .DeleteItem(&item, None)
                .map_err(anyhow::Error::from)?;
            operation.PerformOperations().map_err(anyhow::Error::from)?;
            if operation
                .GetAnyOperationsAborted()
                .map_err(anyhow::Error::from)?
                .as_bool()
            {
                return Err(std::io::Error::other("Recycle Bin operation was cancelled").into());
            }
        }
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = path;
        Err(std::io::Error::other("System Trash is unsupported on this platform").into())
    }
}
