#[path = "support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod support;
use flowtorrent::platform::instance::{InstanceGuard, InstanceRole, forward_open_to};
use std::{path::PathBuf, time::Duration};

#[cfg(target_os = "macos")]
#[test]
fn macos_socket_is_beneath_an_os_owned_private_user_temp_parent() -> anyhow::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let root = support::tempdir()?;
    let InstanceRole::Primary(primary) = InstanceGuard::acquire(&lifecycle::paths(root.path()))?
    else {
        panic!("first owner");
    };
    let socket = std::path::Path::new(primary.endpoint_name());
    let parent = socket.parent().unwrap().parent().unwrap();
    let metadata = std::fs::symlink_metadata(parent)?;
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    // SAFETY: geteuid has no pointer arguments or preconditions.
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    assert_eq!(
        metadata.mode() & 0o777,
        0o700,
        "another user can pre-create the predictable socket directory"
    );
    assert!(primary.endpoint_name().len() < 104);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_guard_forwards_paths_once_in_order_and_releases_only_on_drop() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let InstanceRole::Primary(mut primary) = InstanceGuard::acquire(&paths)? else {
        panic!("first owner");
    };
    assert!(matches!(
        InstanceGuard::acquire(&paths)?,
        InstanceRole::Secondary
    ));
    let mut incoming = primary.listen()?;
    let expected = vec![
        root.path().join("with space.torrent"),
        root.path().join("кириллица.torrent"),
    ];
    forward_open_to(&paths, &expected).await?;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), incoming.recv())
            .await?
            .unwrap(),
        expected
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(150), incoming.recv())
            .await
            .is_err()
    );
    forward_open_to(&paths, &[]).await?;
    assert!(incoming.recv().await.unwrap().is_empty());
    assert!(matches!(
        InstanceGuard::acquire(&paths)?,
        InstanceRole::Secondary
    ));
    primary.stop().await;
    drop(primary);
    assert!(matches!(
        InstanceGuard::acquire(&paths)?,
        InstanceRole::Primary(_)
    ));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn secondary_waits_for_listener_without_starting_another_owner() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let InstanceRole::Primary(mut primary) = InstanceGuard::acquire(&paths)? else {
        panic!("first owner");
    };
    let client_paths = paths.clone();
    let expected = vec![root.path().join("delayed.torrent")];
    let sent = expected.clone();
    let forwarding = tokio::spawn(async move { forward_open_to(&client_paths, &sent).await });
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!forwarding.is_finished());
    assert!(matches!(
        InstanceGuard::acquire(&paths)?,
        InstanceRole::Secondary
    ));
    let mut incoming = primary.listen()?;
    forwarding.await??;
    assert_eq!(incoming.recv().await.unwrap(), expected);
    Ok(())
}

#[tokio::test]
async fn invalid_or_overlarge_messages_do_not_end_the_primary() -> anyhow::Result<()> {
    use interprocess::local_socket::tokio::{Stream, prelude::*};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let InstanceRole::Primary(mut primary) = InstanceGuard::acquire(&paths)? else {
        panic!("first owner");
    };
    let mut incoming = primary.listen()?;
    for body in [
        b"broken json".to_vec(),
        vec![0; 300_000],
        serde_json::to_vec(&serde_json::json!({"version": 999, "paths": []}))?,
    ] {
        let mut conn = Stream::connect(primary.connection_name()?).await?;
        conn.write_all(&(body.len() as u32).to_be_bytes()).await?;
        if body.len() < 256 * 1024 {
            conn.write_all(&body).await?;
        }
        assert_eq!(conn.read_u8().await?, 0);
    }
    assert!(
        forward_open_to(&paths, &[PathBuf::from("relative.torrent")])
            .await
            .is_err()
    );
    assert!(
        forward_open_to(&paths, &vec![root.path().join("file.torrent"); 65])
            .await
            .is_err()
    );
    let valid = vec![root.path().join("valid.torrent")];
    forward_open_to(&paths, &valid).await?;
    assert_eq!(incoming.recv().await.unwrap(), valid);
    Ok(())
}

#[test]
fn instance_child_fixture() -> anyhow::Result<()> {
    let Some(root) = std::env::var_os("FLOWTORRENT_INSTANCE_FIXTURE") else {
        return Ok(());
    };
    let root = PathBuf::from(root);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let InstanceRole::Primary(mut owner) =
                InstanceGuard::acquire(&lifecycle::paths(&root))?
            else {
                anyhow::bail!("child must own the instance");
            };
            let mut receiver = owner.listen()?;
            std::fs::write(root.join("ready"), b"ready")?;
            let paths = receiver
                .recv()
                .await
                .ok_or_else(|| anyhow::anyhow!("listener ended"))?;
            std::fs::write(root.join("received.json"), serde_json::to_vec(&paths)?)?;
            while !root.join("stop").exists() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            owner.stop().await;
            Ok::<_, anyhow::Error>(())
        })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn separate_process_delivers_one_batch_and_owns_the_guard_until_exit() -> anyhow::Result<()> {
    use std::process::{Child, Command, Stdio};
    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let mut child = OwnedChild(
        Command::new(std::env::current_exe()?)
            .args(["--exact", "instance_child_fixture", "--nocapture"])
            .env("FLOWTORRENT_INSTANCE_FIXTURE", root.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()?,
    );
    tokio::time::timeout(Duration::from_secs(60), async {
        while !root.path().join("ready").exists() {
            anyhow::ensure!(
                child.0.try_wait()?.is_none(),
                "fixture exited before acquiring the guard"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    assert!(matches!(
        InstanceGuard::acquire(&paths)?,
        InstanceRole::Secondary
    ));
    let expected = vec![
        root.path().join("separate process.torrent"),
        root.path().join("другой.torrent"),
    ];
    forward_open_to(&paths, &expected).await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !root.path().join("received.json").exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    assert_eq!(
        serde_json::from_slice::<Vec<PathBuf>>(&std::fs::read(root.path().join("received.json"))?)?,
        expected
    );
    assert!(matches!(
        InstanceGuard::acquire(&paths)?,
        InstanceRole::Secondary
    ));
    std::fs::write(root.path().join("stop"), b"stop")?;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(status) = child.0.try_wait()? {
                anyhow::ensure!(status.success(), "fixture failed");
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    assert!(matches!(
        InstanceGuard::acquire(&paths)?,
        InstanceRole::Primary(_)
    ));
    Ok(())
}
