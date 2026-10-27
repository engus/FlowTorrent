use flowtorrent::platform::state::BackgroundNotice;
#[test]
fn background_explanation_is_once_and_corrupt_state_is_preserved() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("shell.json");
    let mut notice = BackgroundNotice::load(path.clone())?;
    assert!(!notice.was_shown());
    assert!(!path.exists());
    notice.mark_shown()?;
    assert!(BackgroundNotice::load(path.clone())?.was_shown());
    for bytes in [
        b"broken".as_slice(),
        b"{\"schema_version\":999,\"background_explained\":true}".as_slice(),
    ] {
        std::fs::write(&path, bytes)?;
        assert!(BackgroundNotice::load(path.clone()).is_err());
        assert_eq!(std::fs::read(&path)?, bytes);
    }
    Ok(())
}

#[tokio::test]
async fn a_slow_native_bridge_does_not_drain_the_bounded_instance_queue() -> anyhow::Result<()> {
    use flowtorrent::platform::{PlatformEvent, forward_instance_events};
    use std::{path::PathBuf, time::Duration};
    let (input, incoming) = tokio::sync::mpsc::channel(32);
    let (native, received) = std::sync::mpsc::channel();
    let pump = tokio::spawn(forward_instance_events(incoming, native));
    input.send(vec![PathBuf::from("first")]).await?;
    input.send(vec![PathBuf::from("second")]).await?;
    tokio::time::sleep(Duration::from_millis(30)).await;
    let PlatformEvent::QueuedOpen { paths, delivered } = received.try_recv()? else {
        panic!("batch expected");
    };
    assert_eq!(paths, [PathBuf::from("first")]);
    assert!(received.try_recv().is_err());
    for _ in 0..31 {
        input.try_send(Vec::new())?;
    }
    assert!(input.try_send(Vec::new()).is_err());
    delivered.send(()).unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    let PlatformEvent::QueuedOpen { paths, .. } = received.try_recv()? else {
        panic!("batch expected");
    };
    assert_eq!(paths, [PathBuf::from("second")]);
    pump.abort();
    Ok(())
}
