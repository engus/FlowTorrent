#[path = "support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod support;

#[tokio::test]
async fn shutdown_timeout_is_an_explicit_shutdown_failure() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let mut handle = lifecycle::start(
        root.path(),
        std::sync::Arc::new(flowtorrent::app::storage::SessionStore::new(paths)),
    )
    .await?;
    let result = flowtorrent::app::lifecycle::wait_for_shutdown(
        &mut handle.snapshots,
        std::time::Duration::from_millis(10),
    )
    .await;
    lifecycle::stop(handle).await?;
    assert!(
        result.is_err(),
        "an active controller was incorrectly reported as stopped"
    );
    assert!(matches!(
        result.unwrap_err(),
        flowtorrent::error::AppError::Lifecycle {
            phase: flowtorrent::error::FailurePhase::Shutdown,
            ..
        }
    ));
    Ok(())
}
