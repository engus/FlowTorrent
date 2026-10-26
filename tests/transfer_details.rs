#[path = "support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod support;
use flowtorrent::{
    app::{commands::AppCommand, storage::SessionStore},
    domain::*,
    engine::{TorrentEngine, librqbit::LibrqbitEngine},
};
use std::sync::Arc;
#[tokio::test]
async fn details_report_actual_selected_file_progress_and_hide_when_not_visible()
-> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let mut handle =
        lifecycle::start(root.path(), Arc::new(SessionStore::new(paths.clone()))).await?;
    let payload = support::payload();
    let metadata = support::torrent_bytes(&payload, false);
    let id = lifecycle::add(
        &mut handle,
        &root.path().join("input.torrent"),
        &paths.download_dir,
        &metadata,
        None,
    )
    .await?;
    handle
        .commands
        .send(AppCommand::InspectDetails(Some(id)))
        .await?;
    let snapshot = lifecycle::until(&mut handle, |s| s.details.is_some()).await?;
    let detail = snapshot.details.unwrap();
    assert_eq!(detail.id, id);
    assert!(!detail.files.is_empty());
    assert!(
        detail
            .files
            .iter()
            .all(|f| f.checked_bytes <= f.length && f.selected)
    );
    handle.commands.send(AppCommand::SetVisible(false)).await?;
    lifecycle::until(&mut handle, |s| !s.visible && s.details.is_none()).await?;
    lifecycle::stop(handle).await
}
#[tokio::test]
async fn engine_details_are_absent_for_unknown_ids() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let mut engine =
        LibrqbitEngine::with_options(root.path().into(), support::local_options(), vec![]).await?;
    assert!(engine.details(TorrentId([0; 20])).await?.is_none());
    engine.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn global_stop_persists_every_torrent_even_when_one_is_inspected() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let store = Arc::new(SessionStore::new(paths.clone()));
    let mut handle = lifecycle::start(root.path(), store.clone()).await?;
    let a = support::torrent_bytes(&support::payload(), false);
    let b = support::torrent_bytes(&support::payload(), true);
    let first = paths.download_dir.join("first");
    let second = paths.download_dir.join("second");
    std::fs::create_dir_all(&first)?;
    std::fs::create_dir_all(&second)?;
    let id = lifecycle::add(
        &mut handle,
        &root.path().join("a.torrent"),
        &first,
        &a,
        None,
    )
    .await?;
    lifecycle::add(
        &mut handle,
        &root.path().join("b.torrent"),
        &second,
        &b,
        None,
    )
    .await?;
    handle
        .commands
        .send(AppCommand::InspectDetails(Some(id)))
        .await?;
    handle
        .commands
        .send(AppCommand::SetAllActive(false))
        .await?;
    lifecycle::until(&mut handle, |s| {
        s.torrents.len() == 2
            && s.torrents
                .iter()
                .all(|r| r.desired_state == DesiredState::Paused)
    })
    .await?;
    lifecycle::until(&mut handle, |s| {
        s.torrents
            .iter()
            .all(|r| matches!(r.state, TransferState::Paused | TransferState::Completed))
    })
    .await?;
    while handle.events.try_recv().is_ok() {}
    handle
        .commands
        .send(AppCommand::SetAllActive(false))
        .await?;
    while let Some(event) = handle.events.recv().await {
        if let flowtorrent::app::AppEvent::CommandFinished {
            operation: flowtorrent::app::commands::AppOperation::SetAllActive,
            success,
        } = event
        {
            assert!(
                success,
                "repeated Stop all must not turn stopped torrents into errors"
            );
            break;
        }
    }
    use flowtorrent::app::storage::SessionRepository;
    assert!(
        store
            .load()?
            .torrents
            .iter()
            .all(|r| r.desired_state == DesiredState::Paused)
    );
    lifecycle::stop(handle).await
}
