#[path = "support/lifecycle.rs"]
mod lifecycle;
#[allow(dead_code)]
mod support;
use flowtorrent::{
    app::{
        commands::AppCommand,
        storage::{SessionRepository, SessionStore},
    },
    domain::*,
    error::AppResult,
};
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct HookStore {
    inner: SessionStore,
    root: PathBuf,
    action: String,
    phase: String,
    tripped: AtomicBool,
}
impl HookStore {
    fn boundary(&self) {
        std::fs::write(self.root.join("boundary"), b"ready").unwrap();
        loop {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
impl SessionRepository for HookStore {
    fn load(&self) -> AppResult<Manifest> {
        self.inner.load()
    }
    fn save(&self, manifest: &Manifest) -> AppResult<()> {
        let matches = match self.action.as_str() {
            "add" => manifest.torrents.len() == 1,
            "pause" => manifest
                .torrents
                .first()
                .is_some_and(|r| r.desired_state == DesiredState::Paused),
            "forget" => manifest.torrents.is_empty(),
            _ => false,
        };
        let trigger = matches && !self.tripped.swap(true, Ordering::SeqCst);
        if trigger && self.phase == "before" {
            self.boundary();
        }
        self.inner.save(manifest)?;
        if trigger && self.phase == "after" {
            self.boundary();
        }
        Ok(())
    }
}

#[test]
fn crash_child_fixture() -> anyhow::Result<()> {
    let Some(root) = std::env::var_os("FLOWTORRENT_CRASH_ROOT") else {
        return Ok(());
    };
    let root = PathBuf::from(root);
    let action = std::env::var("FLOWTORRENT_CRASH_ACTION")?;
    let store = Arc::new(HookStore {
        inner: SessionStore::new(lifecycle::paths(&root)),
        root: root.clone(),
        action: action.clone(),
        phase: std::env::var("FLOWTORRENT_CRASH_PHASE")?,
        tripped: AtomicBool::new(false),
    });
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?
        .block_on(async {
            let mut handle = lifecycle::start(&root, store.clone()).await?;
            if action == "add" {
                lifecycle::add(
                    &mut handle,
                    &root.join("input.torrent"),
                    &root.join("downloads"),
                    &support::torrent_bytes(&support::payload(), false),
                    None,
                )
                .await?;
            } else {
                let id = store.load()?.torrents[0].id;
                lifecycle::until(&mut handle, |s| {
                    s.torrents.iter().any(|r| {
                        r.id == id
                            && !matches!(r.state, TransferState::Checking | TransferState::Paused)
                    })
                })
                .await?;
                handle
                    .commands
                    .send(if action == "pause" {
                        AppCommand::Pause(id)
                    } else {
                        AppCommand::Forget(id)
                    })
                    .await?;
            }
            loop {
                let _ = handle.events.recv().await;
            }
            #[allow(unreachable_code)]
            Ok::<_, anyhow::Error>(())
        })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn killed_add_pause_forget_preserve_only_durable_intentions() -> anyhow::Result<()> {
    for action in ["add", "pause", "forget"] {
        for phase in ["before", "after"] {
            let root = support::tempdir()?;
            let store = Arc::new(SessionStore::new(lifecycle::paths(root.path())));
            std::fs::write(root.path().join("outside.txt"), b"foreign untouched")?;
            if action != "add" {
                let mut handle = lifecycle::start(root.path(), store.clone()).await?;
                lifecycle::add(
                    &mut handle,
                    &root.path().join("input.torrent"),
                    &root.path().join("downloads"),
                    &support::torrent_bytes(&support::payload(), false),
                    None,
                )
                .await?;
                lifecycle::stop(handle).await?;
                std::fs::remove_file(root.path().join("input.torrent"))?;
            }
            let log = std::fs::File::create(root.path().join("child.log"))?;
            let mut child = OwnedChild(
                Command::new(std::env::current_exe()?)
                    .args(["--exact", "crash_child_fixture", "--nocapture"])
                    .env("FLOWTORRENT_CRASH_ROOT", root.path())
                    .env("FLOWTORRENT_CRASH_ACTION", action)
                    .env("FLOWTORRENT_CRASH_PHASE", phase)
                    .stdout(Stdio::from(log.try_clone()?))
                    .stderr(Stdio::from(log))
                    .spawn()?,
            );
            tokio::time::timeout(Duration::from_secs(120), async {
                loop {
                    if root.path().join("boundary").exists() {
                        break;
                    }
                    if let Some(status) = child.0.try_wait()? {
                        anyhow::bail!(
                            "{action}/{phase}: child exited {status}: {}",
                            std::fs::read_to_string(root.path().join("child.log"))?
                        );
                    }
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
                Ok::<_, anyhow::Error>(())
            })
            .await
            .with_context(|| format!("waiting for {action}/{phase}"))??;
            child.0.kill()?;
            child.0.wait()?;
            let manifest = store.load()?;
            let expected = match (action, phase) {
                ("add", "before") | ("forget", "after") => 0,
                _ => 1,
            };
            assert_eq!(manifest.torrents.len(), expected, "{action}/{phase}");
            if action == "pause" {
                assert_eq!(
                    manifest.torrents[0].desired_state,
                    if phase == "after" {
                        DesiredState::Paused
                    } else {
                        DesiredState::Active
                    }
                );
            }
            assert_eq!(
                std::fs::read(root.path().join("outside.txt"))?,
                b"foreign untouched"
            );
            let mut handle = lifecycle::start(root.path(), store.clone()).await?;
            handle.commands.send(AppCommand::SetVisible(false)).await?;
            let state = lifecycle::until(&mut handle, |s| {
                !s.visible
                    && s.torrents.len() == expected
                    && s.torrents
                        .iter()
                        .all(|r| r.state != TransferState::Checking)
            })
            .await?;
            if expected == 1 {
                assert_ne!(
                    state.torrents[0].state,
                    TransferState::Error,
                    "{action}/{phase}: {:?}",
                    state.torrents[0].error
                );
            }
            if action == "pause" && phase == "after" {
                assert_eq!(state.torrents[0].state, TransferState::Paused);
            }
            lifecycle::stop(handle).await?;
        }
    }
    Ok(())
}
use anyhow::Context;
