use anyhow::Result;
use flowtorrent::{
    app::{
        AppController, AppHandle, AppSnapshot, commands::AppCommand, storage::SessionRepository,
    },
    domain::*,
    engine::librqbit::LibrqbitEngine,
    paths::AppPaths,
    settings::SettingsStore,
};
use std::{path::Path, sync::Arc, time::Duration};

pub fn paths(root: &Path) -> AppPaths {
    AppPaths {
        config_file: root.join("state/settings.json"),
        data_dir: root.join("state"),
        metadata_dir: root.join("state/metadata"),
        manifest_file: root.join("state/session.json"),
        download_dir: root.join("downloads"),
    }
}
pub async fn start(root: &Path, store: Arc<dyn SessionRepository>) -> Result<AppHandle> {
    let paths = paths(root);
    std::fs::create_dir_all(&paths.download_dir)?;
    let engine = LibrqbitEngine::with_options(
        paths.download_dir.clone(),
        super::support::local_options(),
        vec![],
    )
    .await?;
    Ok(AppController::spawn(
        Box::new(engine),
        store,
        Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        )),
        paths,
    ))
}
pub async fn until(
    handle: &mut AppHandle,
    predicate: impl Fn(&AppSnapshot) -> bool,
) -> Result<AppSnapshot> {
    let result = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let snapshot = handle.snapshots.borrow().clone();
            if predicate(&snapshot) {
                return Ok(snapshot);
            }
            tokio::select! {result=handle.snapshots.changed()=>result?,_=handle.events.recv()=>{}}
        }
    })
    .await;
    match result {
        Ok(snapshot) => snapshot,
        Err(error) => Err(anyhow::anyhow!(
            "{error}; last snapshot: {:?}",
            handle.snapshots.borrow()
        )),
    }
}
pub async fn add(
    handle: &mut AppHandle,
    source: &Path,
    target: &Path,
    metadata: &[u8],
    selection: Option<Vec<FileId>>,
) -> Result<TorrentId> {
    std::fs::write(source, metadata)?;
    handle
        .commands
        .send(AppCommand::OpenTorrent(source.into()))
        .await?;
    let preview = until(handle, |s| s.preview.is_some())
        .await?
        .preview
        .unwrap();
    let id = preview.id;
    let selection = selection.unwrap_or_else(|| preview.default_selection());
    handle
        .commands
        .send(AppCommand::ConfirmAdd(AddRequest::new(
            id,
            target.into(),
            selection,
        )))
        .await?;
    until(handle, |s| {
        s.preview.is_none() && s.torrents.iter().any(|row| row.id == id)
    })
    .await?;
    Ok(id)
}
pub async fn stop(mut handle: AppHandle) -> Result<()> {
    handle.commands.send(AppCommand::Shutdown).await?;
    tokio::time::timeout(Duration::from_secs(20), async {
        while let Some(event) = handle.events.recv().await {
            if matches!(event, flowtorrent::app::AppEvent::ShutdownReady) {
                return;
            }
        }
    })
    .await?;
    Ok(())
}
