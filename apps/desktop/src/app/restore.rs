use crate::{
    domain::*,
    engine::TorrentEngine,
    error::{AppError, AppResult},
    paths::AppPaths,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub struct RestoreReport {
    pub checking: BTreeSet<TorrentId>,
    pub errors: BTreeMap<TorrentId, UserError>,
}

/// Register each authoritative record paused. Hash checks continue in the
/// engine; the controller can handle commands while waiting to resume it.
pub async fn restore_session(
    engine: &mut dyn TorrentEngine,
    manifest: &Manifest,
    paths: &AppPaths,
) -> RestoreReport {
    let mut report = RestoreReport::default();
    for record in &manifest.torrents {
        match restore_record(engine, record, paths).await {
            Ok(()) => {
                report.checking.insert(record.id);
            }
            Err(error) => {
                report.errors.insert(record.id, error.user_error());
            }
        }
    }
    report
}

pub(super) async fn restore_record(
    engine: &mut dyn TorrentEngine,
    record: &TorrentRecord,
    paths: &AppPaths,
) -> AppResult<()> {
    if record.metadata_path != paths.metadata_dir.join(format!("{}.torrent", record.id)) {
        return Err(AppError::CorruptState {
            path: paths.manifest_file.clone(),
            detail: "Metainfo path is outside the owned registry".into(),
        });
    }
    let preview = engine.inspect(&record.metadata_path).await?;
    if preview.id != record.id || preview.name != record.name {
        return Err(AppError::CorruptState {
            path: record.metadata_path.clone(),
            detail: "Saved metainfo does not match its ownership record".into(),
        });
    }
    engine
        .add(
            &preview,
            &AddRequest::new(
                record.id,
                record.base_dir.clone(),
                record.selected_files.clone(),
            ),
            AddMode::RestorePaused,
        )
        .await?;
    Ok(())
}
