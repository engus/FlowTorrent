use crate::{
    app::AppSnapshot,
    error::{AppError, AppResult, FailurePhase},
};
use std::time::Duration;

pub async fn wait_for_shutdown(
    observer: &mut tokio::sync::watch::Receiver<AppSnapshot>,
    deadline: Duration,
) -> AppResult<()> {
    tokio::time::timeout(deadline, async {
        while observer.changed().await.is_ok() {}
    })
    .await
    .map_err(|error| AppError::Lifecycle {
        phase: FailurePhase::Shutdown,
        detail: error.to_string(),
    })?;
    Ok(())
}
