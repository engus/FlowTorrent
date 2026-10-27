use crate::app::targets::{PreparedAdd, SharedFile};
use anyhow::{Context, Result};
use librqbit::{
    ManagedTorrentShared, TorrentMetadata,
    storage::{BoxStorageFactory, StorageFactory, StorageFactoryExt, TorrentStorage},
};
use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub(super) struct SecureStorageFactory {
    prepared: Arc<PreparedAdd>,
}

impl SecureStorageFactory {
    pub fn new(prepared: Arc<PreparedAdd>) -> Self {
        Self { prepared }
    }
}

impl StorageFactory for SecureStorageFactory {
    type Storage = SecureStorage;
    fn create(
        &self,
        _shared: &ManagedTorrentShared,
        metadata: &TorrentMetadata,
    ) -> Result<Self::Storage> {
        anyhow::ensure!(
            metadata.file_infos.len() == self.prepared.source_files.len(),
            "prepared file count differs from metainfo"
        );
        for (actual, expected) in metadata.file_infos.iter().zip(&self.prepared.source_files) {
            anyhow::ensure!(
                actual.relative_filename == expected.relative_path
                    && actual.len == expected.length
                    && actual.attrs.padding == expected.is_padding,
                "prepared file differs from metainfo"
            );
        }
        Ok(SecureStorage {
            files: Mutex::new(self.prepared.file_handles()?),
        })
    }
    fn clone_box(&self) -> BoxStorageFactory {
        self.clone().boxed()
    }
}

pub(super) struct SecureStorage {
    files: Mutex<Vec<Option<SharedFile>>>,
}

impl SecureStorage {
    fn file(&self, id: usize) -> Result<Option<SharedFile>> {
        self.files
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .context("unknown storage file index")
    }
}

impl TorrentStorage for SecureStorage {
    fn init(&mut self, _shared: &ManagedTorrentShared, _metadata: &TorrentMetadata) -> Result<()> {
        Ok(())
    }
    fn pread_exact(&self, id: usize, offset: u64, buffer: &mut [u8]) -> Result<()> {
        if let Some(file) = self.file(id)? {
            let mut pinned = file.lock().unwrap();
            let file = pinned.reader();
            file.seek(SeekFrom::Start(offset))?;
            file.read_exact(buffer)?;
        } else {
            buffer.fill(0);
        }
        Ok(())
    }
    fn pwrite_all(&self, id: usize, offset: u64, buffer: &[u8]) -> Result<()> {
        if let Some(file) = self.file(id)? {
            let mut pinned = file.lock().unwrap();
            let file = pinned.writer()?;
            file.seek(SeekFrom::Start(offset))?;
            file.write_all(buffer)?;
        }
        Ok(())
    }
    fn ensure_file_length(&self, id: usize, length: u64) -> Result<()> {
        if let Some(file) = self.file(id)? {
            let mut pinned = file.lock().unwrap();
            if pinned.reader().metadata()?.len() == length {
                return Ok(());
            }
            let file = pinned.writer()?;
            #[cfg(windows)]
            mark_sparse(file);
            file.set_len(length)?;
        }
        Ok(())
    }
    fn remove_file(&self, _id: usize, _filename: &Path) -> Result<()> {
        anyhow::bail!("payload deletion is disabled")
    }
    fn remove_directory_if_empty(&self, _path: &Path) -> Result<()> {
        anyhow::bail!("payload deletion is disabled")
    }
    fn take(&self) -> Result<Box<dyn TorrentStorage>> {
        let files = std::mem::take(&mut *self.files.lock().unwrap());
        Ok(Box::new(Self {
            files: Mutex::new(files),
        }))
    }
}

#[cfg(windows)]
fn mark_sparse(file: &std::fs::File) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::{IO::DeviceIoControl, Ioctl::FSCTL_SET_SPARSE};
    let mut returned = 0;
    // SAFETY: the handle is held by a live locked File. No buffers or
    // OVERLAPPED state are supplied for this synchronous control operation.
    let result = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            FSCTL_SET_SPARSE,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        tracing::debug!(error=%std::io::Error::last_os_error(),"filesystem did not enable sparse allocation");
    }
}
