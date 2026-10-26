use flowtorrent::platform::login::{launch_agent, write_entry};
#[test]
fn dedicated_entry_escapes_paths_and_preserves_foreign_files() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("entry.plist");
    let contents = launch_agent(std::path::Path::new(
        "/Applications/A&B <test>.app/Contents/MacOS/FlowTorrent",
    ))?;
    assert!(contents.contains("A&amp;B &lt;test&gt;"));
    write_entry(&path, &contents, true)?;
    assert_eq!(std::fs::read_to_string(&path)?, contents);
    write_entry(&path, &contents, false)?;
    assert!(!path.exists());
    std::fs::write(&path, "foreign")?;
    assert!(write_entry(&path, &contents, true).is_err());
    assert!(write_entry(&path, &contents, false).is_err());
    assert_eq!(std::fs::read_to_string(&path)?, "foreign");
    Ok(())
}
#[cfg(unix)]
#[test]
fn login_entry_never_follows_a_symlink() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let target = root.path().join("foreign");
    let entry = root.path().join("entry");
    std::fs::write(&target, "foreign")?;
    std::os::unix::fs::symlink(&target, &entry)?;
    assert!(write_entry(&entry, "owned", true).is_err());
    assert!(write_entry(&entry, "owned", false).is_err());
    assert_eq!(std::fs::read_to_string(target)?, "foreign");
    Ok(())
}
#[path = "support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod support;
#[tokio::test]
async fn failed_settings_save_restores_actual_login_entry_instead_of_stale_preference()
-> anyhow::Result<()> {
    use flowtorrent::{
        app::{
            AppController, AppEvent,
            commands::{AppCommand, AppOperation},
            storage::SessionStore,
        },
        engine::librqbit::LibrqbitEngine,
        platform::login::{LoginChange, LoginProvider},
        settings::{Settings, SettingsStore},
    };
    use std::sync::{Arc, Mutex};
    struct Login(Arc<Mutex<bool>>);
    impl LoginProvider for Login {
        fn apply(&self, enabled: bool) -> flowtorrent::error::AppResult<Box<dyn LoginChange>> {
            let mut value = self.0.lock().unwrap();
            let previous = *value;
            *value = enabled;
            let cell = self.0.clone();
            Ok(Box::new(move || {
                *cell.lock().unwrap() = previous;
                Ok(())
            }))
        }
    }
    let root = tempfile::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.download_dir)?;
    let blocked = root.path().join("blocked");
    std::fs::write(&blocked, "owned fixture blocker")?;
    let actual = Arc::new(Mutex::new(true));
    let engine =
        LibrqbitEngine::with_options(paths.download_dir.clone(), support::local_options(), vec![])
            .await?;
    let mut handle = AppController::spawn_with_services(
        Box::new(engine),
        Arc::new(SessionStore::new(paths.clone())),
        Arc::new(SettingsStore::new(
            blocked.join("settings.json"),
            paths.download_dir.clone(),
        )),
        paths.clone(),
        Arc::new(|_| panic!("no Trash expected")),
        Arc::new(Login(actual.clone())),
    );
    lifecycle::until(&mut handle, |s| s.settings_recovery.is_some()).await?;
    let mut settings = Settings::defaults(paths.download_dir.clone());
    settings.ui.launch_at_login = true;
    handle
        .commands
        .send(AppCommand::UpdateSettings(settings))
        .await?;
    while let Some(event) = handle.events.recv().await {
        if let AppEvent::CommandFinished {
            operation: AppOperation::UpdateSettings,
            success,
        } = event
        {
            assert!(!success);
            break;
        }
    }
    assert!(
        *actual.lock().unwrap(),
        "rollback used stale JSON=false instead of previous OS=true"
    );
    assert!(!handle.snapshots.borrow().settings.ui.launch_at_login);
    lifecycle::stop(handle).await
}
