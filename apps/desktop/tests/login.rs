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
    struct Login {
        actual: Arc<Mutex<bool>>,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }
    impl LoginProvider for Login {
        fn apply(&self, enabled: bool) -> flowtorrent::error::AppResult<Box<dyn LoginChange>> {
            self.calls.lock().unwrap().push("apply");
            let mut value = self.actual.lock().unwrap();
            let previous = *value;
            *value = enabled;
            let cell = self.actual.clone();
            let calls = self.calls.clone();
            Ok(Box::new(move || {
                calls.lock().unwrap().push("rollback");
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
    let calls = Arc::new(Mutex::new(Vec::new()));
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
        Arc::new(Login {
            actual: actual.clone(),
            calls: calls.clone(),
        }),
    );
    // Windows reports a file ancestor as NotFound, so recovery is not a startup barrier.
    let mut settings = Settings::defaults(paths.download_dir.clone());
    settings.ui.launch_at_login = true;
    handle
        .commands
        .send(AppCommand::UpdateSettings(settings))
        .await?;
    let success = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        while let Some(event) = handle.events.recv().await {
            if let AppEvent::CommandFinished {
                operation: AppOperation::UpdateSettings,
                success,
            } = event
            {
                return Ok::<_, anyhow::Error>(success);
            }
        }
        anyhow::bail!("controller closed before UpdateSettings finished");
    })
    .await??;
    assert!(!success);
    assert_eq!(*calls.lock().unwrap(), ["apply", "rollback"]);
    assert!(
        *actual.lock().unwrap(),
        "rollback used stale JSON=false instead of previous OS=true"
    );
    assert!(!handle.snapshots.borrow().settings.ui.launch_at_login);
    lifecycle::stop(handle).await
}
