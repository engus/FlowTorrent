use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub fn backup_path(path: &Path) -> AppResult<PathBuf> {
    let mut name = path
        .file_name()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "saved-data path has no filename",
            )
        })?
        .to_os_string();
    name.push(".bak");
    Ok(path.with_file_name(name))
}

fn replace_bytes(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    // tempfile uses atomic replace (MoveFileExW on Windows). Never unlink the
    // destination first: interruption must leave the previous or next file.
    temporary.persist(path).map_err(|error| error.error)?;
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    // Finish serialization before touching the last good version.
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    match fs::read(path) {
        Ok(previous) => {
            serde_json::from_slice::<serde_json::Value>(&previous).map_err(|error| {
                AppError::CorruptState {
                    path: path.into(),
                    detail: error.to_string(),
                }
            })?;
            replace_bytes(&backup_path(path)?, &previous)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    replace_bytes(path, &bytes)
}

pub fn check_schema(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| AppError::CorruptState {
            path: path.into(),
            detail: error.to_string(),
        })?;
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| AppError::CorruptState {
            path: path.into(),
            detail: "missing schema_version".into(),
        })?;
    if version != 1 {
        return Err(AppError::UnsupportedSchema {
            path: path.into(),
            version,
        });
    }
    Ok(())
}
