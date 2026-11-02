use crate::{
    domain::{FilePriority, Manifest},
    error::AppResult,
};

pub trait SessionRepository: Send + Sync {
    fn load(&self) -> AppResult<Manifest>;
    fn save(&self, manifest: &Manifest) -> AppResult<()>;
}

use crate::{
    error::AppError,
    paths::AppPaths,
    persistence::{atomic_write_json, backup_path, check_schema_up_to},
};
use std::{collections::BTreeSet, fs};

pub struct SessionStore {
    paths: AppPaths,
}
impl SessionStore {
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }
    fn validate(&self, manifest: &Manifest) -> AppResult<()> {
        if !matches!(manifest.schema_version, 1..=4) {
            return Err(AppError::UnsupportedSchema {
                path: self.paths.manifest_file.clone(),
                version: manifest.schema_version.into(),
            });
        }
        let mut ids = BTreeSet::new();
        let mut positions = BTreeSet::new();
        for record in &manifest.torrents {
            let selected: BTreeSet<_> = record.selected_files.iter().map(|id| id.0).collect();
            let priorities: BTreeSet<_> = record
                .file_priorities
                .iter()
                .map(|item| item.file.0)
                .collect();
            if (manifest.schema_version < 4 && record.priority != FilePriority::Normal)
                || (manifest.schema_version < 3 && !record.file_priorities.is_empty())
                || priorities.len() != record.file_priorities.len()
                || record
                    .file_priorities
                    .iter()
                    .any(|item| item.priority == FilePriority::Normal)
                || !ids.insert(record.id)
                || !positions.insert(record.position)
                || record.position >= manifest.next_position
                || (manifest.schema_version == 1 && record.selected_files.is_empty())
                || selected.len() != record.selected_files.len()
                || !record.base_dir.is_absolute()
                || record.metadata_path
                    != self
                        .paths
                        .metadata_dir
                        .join(format!("{}.torrent", record.id))
            {
                return Err(AppError::CorruptState {
                    path: self.paths.manifest_file.clone(),
                    detail: "Invalid or duplicate torrent ownership record".into(),
                });
            }
        }
        Ok(())
    }
}
impl SessionRepository for SessionStore {
    fn load(&self) -> AppResult<Manifest> {
        let bytes = match fs::read(&self.paths.manifest_file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if backup_path(&self.paths.manifest_file)?.try_exists()? {
                    return Err(AppError::CorruptState {
                        path: self.paths.manifest_file.clone(),
                        detail: "The session registry is missing but a previous copy exists".into(),
                    });
                }
                return Ok(Manifest::default());
            }
            Err(error) => return Err(error.into()),
        };
        check_schema_up_to(&self.paths.manifest_file, &bytes, 4)?;
        let manifest = serde_json::from_slice(&bytes).map_err(|error| AppError::CorruptState {
            path: self.paths.manifest_file.clone(),
            detail: error.to_string(),
        })?;
        self.validate(&manifest)?;
        Ok(manifest)
    }
    fn save(&self, manifest: &Manifest) -> AppResult<()> {
        self.load()?;
        self.validate(manifest)?;
        atomic_write_json(&self.paths.manifest_file, manifest)
    }
}
