#[path = "support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod support;
use flowtorrent::{
    app::{commands::AppCommand, storage::SessionStore},
    settings::{RecoveryStatus, Settings, SettingsStore},
};
use std::sync::Arc;

#[test]
fn damaged_or_future_preferences_recover_and_save_separately_across_restart() -> anyhow::Result<()>
{
    for original in [b"{ damaged".to_vec(), br#"{"schema_version":99}"#.to_vec()] {
        let root = support::tempdir()?;
        let path = root.path().join("settings.json");
        std::fs::write(&path, &original)?;
        let store = SettingsStore::new(path.clone(), root.path().into());
        let loaded = store.load_for_app()?;
        let recovery = loaded.recovery.unwrap();
        assert_eq!(recovery.original, path);
        assert_eq!(recovery.status, RecoveryStatus::Defaults);
        assert_eq!(loaded.settings, Settings::defaults(root.path().into()));
        let mut changed = loaded.settings;
        changed.notify_on_complete = false;
        store.save_for_app(&changed)?;
        assert_eq!(std::fs::read(&path)?, original);
        assert!(recovery.replacement.is_file());
        let restarted = store.load_for_app()?;
        assert_eq!(restarted.settings, changed);
        assert_eq!(
            restarted.recovery.unwrap().status,
            RecoveryStatus::Recovered
        );
    }
    Ok(())
}

#[test]
fn a_damaged_recovery_file_is_also_preserved() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let path = root.path().join("settings.json");
    std::fs::write(&path, b"broken original")?;
    let store = SettingsStore::new(path.clone(), root.path().into());
    let recovery = store.load_for_app()?.recovery.unwrap();
    std::fs::write(&recovery.replacement, b"broken recovery")?;
    let loaded = store.load_for_app()?;
    assert_eq!(loaded.recovery.unwrap().status, RecoveryStatus::Blocked);
    assert!(store.save_for_app(&loaded.settings).is_err());
    assert_eq!(std::fs::read(path)?, b"broken original");
    assert_eq!(std::fs::read(recovery.replacement)?, b"broken recovery");
    Ok(())
}

#[tokio::test]
async fn corrupt_settings_do_not_disable_torrent_preview() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.data_dir)?;
    std::fs::write(&paths.config_file, b"broken preferences")?;
    let mut app = lifecycle::start(root.path(), Arc::new(SessionStore::new(paths.clone()))).await?;
    let input = root.path().join("input.torrent");
    std::fs::write(&input, support::torrent_bytes(&support::payload(), false))?;
    app.commands.send(AppCommand::OpenTorrent(input)).await?;
    let preview = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        lifecycle::until(&mut app, |s| s.preview.is_some()),
    )
    .await;
    lifecycle::stop(app).await?;
    assert!(
        preview.is_ok(),
        "corrupt settings locked the whole controller"
    );
    assert_eq!(std::fs::read(paths.config_file)?, b"broken preferences");
    Ok(())
}
