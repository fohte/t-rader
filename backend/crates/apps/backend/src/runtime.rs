use futures_util::future::BoxFuture;
use tokio::{sync::watch, task::JoinSet};

use crate::startup::StartupError;

mod cron_trigger_timer;

pub(crate) use cron_trigger_timer::run as run_cron_trigger_timer;

pub(super) async fn supervise(
    run_futures: Vec<BoxFuture<'static, Result<(), StartupError>>>,
    shutdown_tx: watch::Sender<bool>,
) -> Result<(), StartupError> {
    let mut tasks = JoinSet::new();
    for run_future in run_futures {
        tasks.spawn(run_future);
    }

    let Some(first_completion) = tasks.join_next().await else {
        return Err(StartupError::Runtime(
            "backend runtime setup did not start any components".to_string(),
        ));
    };
    let mut result = completion_result(first_completion);
    let _ = shutdown_tx.send(true);

    while let Some(completion) = tasks.join_next().await {
        if let Err(error) = completion_result(completion) {
            if result.is_ok() {
                result = Err(error);
            } else {
                tracing::error!(%error, "another backend component failed while shutting down");
            }
        }
    }

    result
}

fn completion_result(
    completion: Result<Result<(), StartupError>, tokio::task::JoinError>,
) -> Result<(), StartupError> {
    match completion {
        Ok(result) => result,
        Err(error) => Err(StartupError::Runtime(format!(
            "backend component task failed: {error}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::future::BoxFuture;
    use tokio::{sync::watch, time::timeout};

    use super::*;
    use crate::signals::wait_for_shutdown;

    #[tokio::test]
    async fn component_error_shuts_down_other_components_and_exits() {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let long_running_component: BoxFuture<'static, Result<(), StartupError>> =
            Box::pin(async move {
                wait_for_shutdown(shutdown_rx).await;
                Ok(())
            });
        let failing_component: BoxFuture<'static, Result<(), StartupError>> =
            Box::pin(async { Err(StartupError::Runtime("listener closed".to_string())) });

        let actual = timeout(
            Duration::from_secs(1),
            supervise(vec![long_running_component, failing_component], shutdown_tx),
        )
        .await
        .map_err(|_| "supervision timed out".to_string())
        .and_then(|result| result.map_err(|error| error.to_string()));

        assert_eq!(
            actual,
            Err("configuration error: listener closed".to_string()),
        );
    }

    #[tokio::test]
    async fn successful_component_completion_shuts_down_other_components() {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let long_running_component: BoxFuture<'static, Result<(), StartupError>> =
            Box::pin(async move {
                wait_for_shutdown(shutdown_rx).await;
                Ok(())
            });
        let completing_component: BoxFuture<'static, Result<(), StartupError>> =
            Box::pin(async { Ok(()) });

        let actual = timeout(
            Duration::from_secs(1),
            supervise(
                vec![long_running_component, completing_component],
                shutdown_tx,
            ),
        )
        .await
        .map_err(|_| "supervision timed out".to_string())
        .and_then(|result| result.map_err(|error| error.to_string()));

        assert_eq!(actual, Ok(()));
    }

    #[tokio::test]
    async fn empty_runtime_reports_a_setup_error() {
        let (shutdown_tx, _) = watch::channel(false);

        let actual = supervise(Vec::new(), shutdown_tx)
            .await
            .map_err(|error| error.to_string());

        assert_eq!(
            actual,
            Err(
                "configuration error: backend runtime setup did not start any components"
                    .to_string()
            ),
        );
    }
}
