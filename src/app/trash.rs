use super::targets::{
    open_base_without_links, parent_directory, reject_hardlink, relative_components,
};
use crate::{
    domain::TorrentId,
    error::{AppError, AppResult},
    persistence::atomic_write_json,
};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::File,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Identity {
    volume: u64,
    index: u64,
    length: u64,
}
pub(crate) fn identity(file: &File) -> AppResult<Identity> {
    let metadata = file.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(Identity {
            volume: metadata.dev(),
            index: metadata.ino(),
            length: if metadata.is_file() {
                metadata.len()
            } else {
                0
            },
        })
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
        };
        let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
        // SAFETY: a live File owns the handle; output has the documented size.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: successful API call initialized the structure.
        let info = unsafe { info.assume_init() };
        Ok(Identity {
            volume: u64::from(info.dwVolumeSerialNumber),
            index: u64::from(info.nFileIndexHigh) << 32 | u64::from(info.nFileIndexLow),
            length: if metadata.is_file() {
                metadata.len()
            } else {
                0
            },
        })
    }
}
pub(crate) fn open_regular(parent: &Dir, name: &Path) -> AppResult<File> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = parent.open_with(name, &options)?.into_std();
    if !file.metadata()?.is_file() {
        return Err(AppError::FileConflict(name.into()));
    }
    reject_hardlink(&file, name)?;
    Ok(file)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub relative: PathBuf,
    pub identity: Identity,
}
#[derive(Serialize, Deserialize)]
struct Journal {
    schema_version: u32,
    id: TorrentId,
    base: PathBuf,
    stage: PathBuf,
    stage_identity: Identity,
    entries: Vec<Entry>,
    #[serde(default)]
    trashed: bool,
}
pub struct TrashStage {
    journal_path: PathBuf,
    journal: Journal,
    root: Dir,
    stage: Dir,
}
impl TrashStage {
    pub(crate) fn create(
        id: TorrentId,
        base: PathBuf,
        root: Dir,
        entries: Vec<Entry>,
        journal_path: &Path,
    ) -> AppResult<Self> {
        if journal_path.try_exists()? {
            return Err(AppError::FileConflict(journal_path.into()));
        }
        let name = format!(
            "FlowTorrent Trash {}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| AppError::InvalidMetadata("clock before epoch".into()))?
                .as_nanos()
        );
        create_private(&root, &base, &name)?;
        let stage = root.open_dir_nofollow(&name)?;
        let journal = Journal {
            schema_version: 1,
            id,
            stage: base.join(&name),
            base,
            stage_identity: identity(&stage.try_clone()?.into_std_file())?,
            entries,
            trashed: false,
        };
        atomic_write_json(journal_path, &journal)?;
        let mut result = Self {
            journal_path: journal_path.into(),
            journal,
            root,
            stage,
        };
        result.complete_staging()?;
        Ok(result)
    }
    pub fn load(path: &Path, id: TorrentId) -> AppResult<Self> {
        let bytes = std::fs::read(path)?;
        crate::persistence::check_schema(path, &bytes)?;
        let journal: Journal = serde_json::from_slice(&bytes)?;
        let name = journal
            .stage
            .file_name()
            .ok_or_else(|| AppError::FileConflict(path.into()))?;
        if journal.schema_version != 1
            || journal.id != id
            || journal.entries.is_empty()
            || journal.stage.parent() != Some(journal.base.as_path())
            || !name.to_string_lossy().starts_with("FlowTorrent Trash ")
        {
            return Err(AppError::FileConflict(path.into()));
        }
        let mut paths = BTreeSet::new();
        for e in &journal.entries {
            relative_components(&e.relative)?;
            if !paths.insert(e.relative.clone()) {
                return Err(AppError::FileConflict(path.into()));
            }
        }
        let root = open_base_without_links(&journal.base)?;
        let stage = root.open_dir_nofollow(name)?;
        if identity(&stage.try_clone()?.into_std_file())? != journal.stage_identity {
            return Err(AppError::FileConflict(journal.stage.clone()));
        }
        let mut result = Self {
            journal_path: path.into(),
            journal,
            root,
            stage,
        };
        result.complete_staging()?;
        Ok(result)
    }
    pub fn path(&self) -> &Path {
        &self.journal.stage
    }
    pub fn recovery_path(path: &Path) -> Option<PathBuf> {
        let j: Journal = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        Some(j.stage)
    }
    fn complete_staging(&mut self) -> AppResult<()> {
        for entry in &self.journal.entries {
            let target_parent = parent_directory(&self.stage, &entry.relative, true)?
                .ok_or_else(|| AppError::UnavailableFolder(self.journal.stage.clone()))?;
            let filename = Path::new(
                entry
                    .relative
                    .file_name()
                    .ok_or_else(|| AppError::FileConflict(entry.relative.clone()))?,
            );
            if target_parent.symlink_metadata(filename).is_ok() {
                let staged = open_regular(&target_parent, filename)?;
                if identity(&staged)? != entry.identity {
                    return Err(AppError::FileConflict(
                        self.journal.stage.join(&entry.relative),
                    ));
                }
                continue;
            }
            let parent = parent_directory(&self.root, &entry.relative, false)?
                .ok_or_else(|| AppError::UnavailableFolder(self.journal.base.clone()))?;
            let file = open_regular(&parent, filename)?;
            if identity(&file)? != entry.identity {
                return Err(AppError::FileConflict(
                    self.journal.base.join(&entry.relative),
                ));
            }
            parent.rename(filename, &target_parent, filename)?;
            let staged = open_regular(&target_parent, filename)?;
            if identity(&staged)? != entry.identity {
                return Err(AppError::FileConflict(
                    self.journal.stage.join(&entry.relative),
                ));
            }
            // The journal already describes both sides before the rename.
            #[cfg(unix)]
            {
                target_parent.try_clone()?.into_std_file().sync_all()?;
                parent.try_clone()?.into_std_file().sync_all()?;
            }
        }
        self.validate()
    }
    fn validate(&self) -> AppResult<()> {
        let named = open_base_without_links(&self.journal.stage)?;
        if identity(&named.into_std_file())? != self.journal.stage_identity {
            return Err(AppError::FileConflict(self.journal.stage.clone()));
        }
        let allowed: BTreeSet<_> = self
            .journal
            .entries
            .iter()
            .map(|e| e.relative.clone())
            .collect();
        let mut stack = vec![(self.stage.try_clone()?, PathBuf::new())];
        let mut found = BTreeSet::new();
        while let Some((dir, prefix)) = stack.pop() {
            for entry in dir.entries()? {
                let entry = entry?;
                let relative = prefix.join(entry.file_name());
                if entry.file_type()?.is_dir() && !entry.file_type()?.is_symlink() {
                    if !allowed.iter().any(|p| p.starts_with(&relative)) {
                        return Err(AppError::FileConflict(relative));
                    }
                    stack.push((dir.open_dir_nofollow(entry.file_name())?, relative));
                } else {
                    let expected = self
                        .journal
                        .entries
                        .iter()
                        .find(|e| e.relative == relative)
                        .ok_or_else(|| AppError::FileConflict(relative.clone()))?;
                    let file = open_regular(&dir, Path::new(&entry.file_name()))?;
                    if identity(&file)? != expected.identity {
                        return Err(AppError::FileConflict(relative));
                    }
                    found.insert(relative);
                }
            }
        }
        if found != allowed {
            return Err(AppError::FileConflict(self.journal.stage.clone()));
        }
        Ok(())
    }
    pub fn finish_with(mut self, trash: impl FnOnce(&Path) -> AppResult<()>) -> AppResult<()> {
        self.validate()?;
        let mut roots = BTreeSet::new();
        for entry in &self.journal.entries {
            let components = relative_components(&entry.relative)?;
            let root = components
                .first()
                .ok_or_else(|| AppError::FileConflict(self.journal.stage.clone()))?;
            roots.insert(root.clone());
        }
        if roots.len() != 1 {
            return Err(AppError::FileConflict(self.journal.stage.clone()));
        }
        let root = roots
            .into_iter()
            .next()
            .ok_or_else(|| AppError::FileConflict(self.journal.stage.clone()))?;
        // Keep the private wrapper pinned. Windows cap directory handles deny
        // renaming themselves; the owned payload child has no live directory pin.
        trash(&self.journal.stage.join(&root))?;
        match self.stage.symlink_metadata(&root) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
            Ok(_) => return Err(AppError::FileConflict(self.journal.stage.join(root))),
        }
        self.journal.trashed = true;
        // Retain the receipt until registry removal is durable. Otherwise a
        // failed save/restart could recreate files already moved to Trash.
        atomic_write_json(&self.journal_path, &self.journal)?;
        // Empty app-owned wrapper only. Cleanup failure cannot undo native Trash.
        if let Err(error) = self.stage.remove_open_dir() {
            tracing::warn!(%error, "could not remove empty Trash staging wrapper");
        }
        Ok(())
    }
    pub fn is_trashed(path: &Path, id: TorrentId) -> AppResult<bool> {
        let bytes = std::fs::read(path)?;
        crate::persistence::check_schema(path, &bytes)?;
        let journal: Journal = serde_json::from_slice(&bytes)?;
        if journal.id != id {
            return Err(AppError::FileConflict(path.into()));
        }
        Ok(journal.trashed)
    }
    pub fn finish_registry_removal(path: &Path, id: TorrentId) -> AppResult<()> {
        if !Self::is_trashed(path, id)? {
            return Err(AppError::FileConflict(path.into()));
        }
        std::fs::remove_file(path)?;
        Ok(())
    }
}
fn create_private(root: &Dir, _base: &Path, name: &str) -> AppResult<()> {
    #[cfg(unix)]
    {
        use cap_std::fs::{DirBuilder, DirBuilderExt};
        let mut builder = DirBuilder::new();
        builder.mode(0o700);
        root.create_dir_with(name, &builder)?;
        Ok(())
    }
    #[cfg(windows)]
    {
        use interprocess::os::windows::security_descriptor::{
            AsSecurityDescriptor, SecurityDescriptor,
        };
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::{
            Security::SECURITY_ATTRIBUTES, Storage::FileSystem::CreateDirectoryW,
        };
        let descriptor = SecurityDescriptor::deserialize(
            widestring::U16CString::from_str("D:P(A;OICI;FA;;;OW)")
                .map_err(|_| AppError::InvalidMetadata("invalid DACL".into()))?
                .as_ucstr(),
        )?;
        let attrs = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.as_sd().cast_mut(),
            bInheritHandle: 0,
        };
        let path = _base
            .join(name)
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        // SAFETY: descriptor and terminated path remain valid throughout the synchronous call.
        if unsafe { CreateDirectoryW(path.as_ptr(), &attrs) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // Opening through the pinned root prevents following a replaced parent.
        root.open_dir_nofollow(name)?;
        Ok(())
    }
}
