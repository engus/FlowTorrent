use crate::{
    domain::*,
    error::{AppError, AppResult},
};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use std::{
    collections::BTreeSet,
    fs::File,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};
use unicode_normalization::UnicodeNormalization;

pub(crate) type SharedFile = Arc<Mutex<PinnedFile>>;

#[derive(Debug)]
pub(crate) struct PinnedFile {
    file: File,
    writable: bool,
}
impl PinnedFile {
    fn new(file: File, writable: bool) -> Self {
        Self { file, writable }
    }
    pub fn reader(&mut self) -> &mut File {
        &mut self.file
    }
    pub fn writer(&mut self) -> std::io::Result<&mut File> {
        if !self.writable {
            self.set_access(true)?;
        }
        Ok(&mut self.file)
    }
    fn set_access(&mut self, writable: bool) -> std::io::Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::io::{AsRawHandle, FromRawHandle};
            use windows_sys::Win32::{
                Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE},
                Storage::FileSystem::{
                    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, ReOpenFile,
                },
            };
            let access = GENERIC_READ | if writable { GENERIC_WRITE } else { 0 };
            // SAFETY: the live File owns this valid handle. ReOpenFile refers
            // to the same file object, so no path or reparse point is followed.
            let reopened = unsafe {
                ReOpenFile(
                    self.file.as_raw_handle(),
                    access,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    0,
                )
            };
            if reopened == INVALID_HANDLE_VALUE || reopened.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            // SAFETY: the successful call returned a new uniquely owned handle.
            self.file = unsafe { File::from_raw_handle(reopened) };
        }
        // Unix does not impose Windows' symmetric read/write sharing checks.
        self.writable = writable;
        Ok(())
    }
}

#[derive(Debug)]
struct Claims {
    files: Vec<Option<SharedFile>>,
    complete: bool,
}

/// Pins the chosen directory and owns handles for files created after confirmation.
/// All subsequent I/O uses those handles, never a reopened concatenated path.
#[derive(Debug)]
pub struct PreparedAdd {
    pub destination: PathBuf,
    root: Arc<Dir>,
    request: AddRequest,
    pub(crate) source_files: Vec<TorrentFile>,
    relative_paths: Vec<PathBuf>,
    restore: bool,
    name: String,
    multi_file: bool,
    claims: Mutex<Claims>,
}

fn bad_path() -> AppError {
    AppError::InvalidMetadata("unsupported or escaping torrent path".into())
}

fn validate_component(name: &str) -> AppResult<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c < ' ' || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return Err(bad_path());
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|tail| {
                matches!(
                    tail,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
    if device {
        return Err(bad_path());
    }
    Ok(())
}

pub(crate) fn relative_components(path: &Path) -> AppResult<Vec<String>> {
    let raw = path.to_str().ok_or_else(bad_path)?;
    let raw_parts: Vec<_> = if cfg!(windows) {
        raw.split(['/', '\\']).collect()
    } else {
        raw.split('/').collect()
    };
    if raw_parts
        .iter()
        .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return Err(bad_path());
    }
    path.components()
        .map(|component| {
            let Component::Normal(name) = component else {
                return Err(bad_path());
            };
            let name = name.to_str().ok_or_else(bad_path)?;
            validate_component(name)?;
            Ok(name.to_owned())
        })
        .collect()
}

pub(crate) fn open_base_without_links(path: &Path) -> AppResult<Dir> {
    if !path.is_absolute() {
        return Err(AppError::UnavailableFolder(path.into()));
    }
    let mut root_path = PathBuf::new();
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => root_path.push(prefix.as_os_str()),
            Component::RootDir => root_path.push(component.as_os_str()),
            Component::Normal(name) => names.push(name.to_owned()),
            _ => return Err(AppError::UnavailableFolder(path.into())),
        }
    }
    let mut dir = Dir::open_ambient_dir(root_path, cap_std::ambient_authority())
        .map_err(|_| AppError::UnavailableFolder(path.into()))?;
    for name in names {
        dir = dir
            .open_dir_nofollow(name)
            .map_err(|_| AppError::UnavailableFolder(path.into()))?;
    }
    Ok(dir)
}

pub(crate) fn parent_directory(
    root: &Dir,
    path: &Path,
    create: bool,
) -> std::io::Result<Option<Dir>> {
    let mut dir = root.try_clone()?;
    let parent = path.parent().unwrap_or(Path::new(""));
    for component in parent.components() {
        let Component::Normal(name) = component else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid relative directory",
            ));
        };
        match dir.open_dir_nofollow(name) {
            Ok(next) => dir = next,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
                match dir.create_dir(name) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error),
                };
                dir = dir.open_dir_nofollow(name)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
    }
    Ok(Some(dir))
}

fn prepare(
    preview: &TorrentPreview,
    request: &AddRequest,
    restore: bool,
) -> AppResult<PreparedAdd> {
    if request.id != preview.id
        || !valid_selection(&preview.files, &request.selected_files)
        || preview.files.is_empty()
    {
        return Err(bad_path());
    }
    let prefix = if preview.multi_file {
        validate_component(&preview.name)?;
        PathBuf::from(&preview.name)
    } else {
        PathBuf::new()
    };
    let mut normalized = BTreeSet::new();
    let mut relative_paths = Vec::new();
    for (index, file) in preview.files.iter().enumerate() {
        if file.id.0 != index {
            return Err(bad_path());
        }
        let components = relative_components(&file.relative_path)?;
        if components.is_empty() {
            return Err(bad_path());
        }
        let key = components
            .join("/")
            .nfc()
            .collect::<String>()
            .to_lowercase();
        if !normalized.insert(key) {
            return Err(bad_path());
        }
        relative_paths.push(prefix.join(&file.relative_path));
    }
    for key in &normalized {
        let mut parent = key.as_str();
        while let Some((next, _)) = parent.rsplit_once('/') {
            if normalized.contains(next) {
                return Err(bad_path());
            }
            parent = next;
        }
    }
    let root = Arc::new(open_base_without_links(&request.base_dir)?);
    for (file, path) in preview.files.iter().zip(&relative_paths) {
        if file.is_padding {
            continue;
        }
        if let Some(parent) = parent_directory(&root, path, false)? {
            match parent.symlink_metadata(path.file_name().ok_or_else(bad_path)?) {
                Ok(metadata)
                    if restore && metadata.is_file() && !metadata.file_type().is_symlink() => {}
                Ok(_) => return Err(AppError::FileConflict(request.base_dir.join(path))),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(PreparedAdd {
        destination: request.base_dir.join(prefix),
        root,
        request: AddRequest::new(
            request.id,
            request.base_dir.clone(),
            request.selected_files.clone(),
        ),
        source_files: preview.files.clone(),
        relative_paths,
        restore,
        name: preview.name.clone(),
        multi_file: preview.multi_file,
        claims: Mutex::new(Claims {
            files: vec![None; preview.files.len()],
            complete: false,
        }),
    })
}

pub fn prepare_add(preview: &TorrentPreview, request: &AddRequest) -> AppResult<PreparedAdd> {
    prepare(preview, request, false)
}
pub(crate) fn prepare_restore(
    preview: &TorrentPreview,
    request: &AddRequest,
) -> AppResult<PreparedAdd> {
    prepare(preview, request, true)
}

impl PreparedAdd {
    pub fn stage_for_trash(
        &self,
        id: TorrentId,
        selected: &[FileId],
        journal: &Path,
    ) -> AppResult<super::trash::TrashStage> {
        if id != self.request.id || !valid_selection(&self.source_files, selected) {
            return Err(bad_path());
        }
        let claims = self.claims.lock().unwrap();
        let mut entries = Vec::new();
        for file in &self.source_files {
            if !selected.contains(&file.id) || file.is_padding {
                continue;
            }
            let claimed = claims
                .files
                .get(file.id.0)
                .and_then(Option::as_ref)
                .ok_or_else(bad_path)?;
            let mut pinned = claimed.lock().unwrap();
            let pinned = pinned.reader();
            reject_hardlink(pinned, &self.relative_paths[file.id.0])?;
            let relative = &self.relative_paths[file.id.0];
            let parent = parent_directory(&self.root, relative, false)?.ok_or_else(bad_path)?;
            let named = super::trash::open_regular(
                &parent,
                Path::new(relative.file_name().ok_or_else(bad_path)?),
            )?;
            let expected = super::trash::identity(pinned)?;
            if super::trash::identity(&named)? != expected {
                return Err(AppError::FileConflict(self.request.base_dir.join(relative)));
            }
            entries.push(super::trash::Entry {
                relative: relative.clone(),
                identity: expected,
            });
        }
        super::trash::TrashStage::create(
            id,
            self.request.base_dir.clone(),
            self.root.try_clone()?,
            entries,
            journal,
        )
    }
    /// Reuse only this process's still-empty, still-named pinned files. User
    /// writes, replacements and new hard links invalidate an abandoned claim.
    pub(crate) fn validate_uncommitted(&self) -> AppResult<()> {
        let claims = self.claims.lock().unwrap();
        for (index, claimed) in claims.files.iter().enumerate() {
            let Some(claimed) = claimed else {
                continue;
            };
            let path = &self.relative_paths[index];
            let conflict = || AppError::FileConflict(self.request.base_dir.join(path));
            let mut file = claimed.lock().unwrap();
            let file = file.reader();
            if file.metadata()?.len() != 0 {
                return Err(conflict());
            }
            reject_hardlink(file, &self.request.base_dir.join(path))?;
            let parent = parent_directory(&self.root, path, false)?.ok_or_else(conflict)?;
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            #[cfg(unix)]
            {
                use cap_std::fs::OpenOptionsExt;
                options.custom_flags(libc::O_NONBLOCK);
            }
            let named = parent
                .open_with(path.file_name().ok_or_else(bad_path)?, &options)
                .map_err(|_| conflict())?
                .into_std();
            if same_file::Handle::from_file(file.try_clone()?)?
                != same_file::Handle::from_file(named)?
            {
                return Err(conflict());
            }
        }
        Ok(())
    }

    pub fn claim_files(&self) -> AppResult<()> {
        let mut claims = self.claims.lock().unwrap();
        if claims.complete {
            return Ok(());
        }
        for (index, (file, path)) in self
            .source_files
            .iter()
            .zip(&self.relative_paths)
            .enumerate()
        {
            if file.is_padding || claims.files[index].is_some() {
                continue;
            }
            let parent = parent_directory(&self.root, path, true)?
                .ok_or_else(|| AppError::UnavailableFolder(self.destination.clone()))?;
            let mut options = OpenOptions::new();
            options.read(true).write(true).follow(FollowSymlinks::No);
            #[cfg(unix)]
            {
                use cap_std::fs::OpenOptionsExt;
                options.custom_flags(libc::O_NONBLOCK);
            }
            if !self.restore {
                options.create_new(true);
            }
            let filename = path.file_name().ok_or_else(bad_path)?;
            #[allow(unused_mut)]
            let mut writable = true;
            let opened = match parent.open_with(filename, &options) {
                #[cfg(windows)]
                Err(error)
                    if self.restore
                        && (error.kind() == std::io::ErrorKind::PermissionDenied
                            || error.raw_os_error() == Some(32)) =>
                {
                    let mut reader_options = OpenOptions::new();
                    reader_options.read(true).follow(FollowSymlinks::No);
                    writable = false;
                    parent.open_with(filename, &reader_options)
                }
                Err(error) if self.restore && error.kind() == std::io::ErrorKind::NotFound => {
                    // The authoritative manifest owns this path. Recreate a
                    // missing file exclusively; a racing new file is a conflict.
                    options.create_new(true);
                    parent.open_with(filename, &options)
                }
                result => result,
            }
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    AppError::FileConflict(self.request.base_dir.join(path))
                } else {
                    error.into()
                }
            })?
            .into_std();
            if !opened.metadata()?.is_file() {
                return Err(AppError::FileConflict(self.request.base_dir.join(path)));
            }
            if self.restore {
                reject_hardlink(&opened, &self.request.base_dir.join(path))?;
            }
            claims.files[index] = Some(Arc::new(Mutex::new(PinnedFile::new(opened, writable))));
        }
        claims.complete = true;
        Ok(())
    }

    /// The controller persists ownership only after this succeeds, before
    /// handing the claimed request to the engine and acknowledging the add.
    pub fn confirm(self: Arc<Self>) -> AppResult<AddRequest> {
        let selected = self.request.selected_files.clone();
        self.confirm_selection(selected)
    }

    pub(crate) fn confirm_selection(
        self: Arc<Self>,
        selected: Vec<FileId>,
    ) -> AppResult<AddRequest> {
        if !valid_selection(&self.source_files, &selected) {
            return Err(bad_path());
        }
        self.claim_files()?;
        let mut request = self.request.clone();
        request.selected_files = selected;
        request.prepared = Some(self);
        Ok(request)
    }

    pub(crate) fn file_handles(&self) -> AppResult<Vec<Option<SharedFile>>> {
        self.claim_files()?;
        Ok(self.claims.lock().unwrap().files.clone())
    }

    pub(crate) fn release_writers(&self) -> AppResult<()> {
        for file in self.claims.lock().unwrap().files.iter().flatten() {
            let mut file = file.lock().unwrap();
            if file.writable {
                file.set_access(false)?;
            }
        }
        Ok(())
    }

    pub(crate) fn matches(&self, preview: &TorrentPreview, request: &AddRequest) -> bool {
        !self.restore
            && self.name == preview.name
            && self.multi_file == preview.multi_file
            && self.request.id == request.id
            && self.request.base_dir == request.base_dir
            && valid_selection(&self.source_files, &request.selected_files)
            && self.source_files == preview.files
    }
}

fn valid_selection(files: &[TorrentFile], selected: &[FileId]) -> bool {
    !selected.is_empty()
        && selected
            .iter()
            .map(|id| id.0)
            .collect::<BTreeSet<_>>()
            .len()
            == selected.len()
        && selected.iter().all(|id| {
            files
                .get(id.0)
                .is_some_and(|file| file.id == *id && !file.is_padding)
        })
}

pub(crate) fn reject_hardlink(file: &File, path: &Path) -> AppResult<()> {
    #[cfg(unix)]
    let links = {
        use std::os::unix::fs::MetadataExt;
        file.metadata()?.nlink()
    };
    #[cfg(windows)]
    let links = {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
        };
        let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
        // SAFETY: the live File owns a valid handle and the output buffer has
        // the documented size. Only read it after the API reports success.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: the successful call initialized every field in the buffer.
        u64::from(unsafe { info.assume_init() }.nNumberOfLinks)
    };
    if links > 1 {
        return Err(AppError::FileConflict(path.into()));
    }
    Ok(())
}
