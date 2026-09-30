use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use gateway_jquants::JQuantsClient;
use graphile_worker::WorkerContext;
use tokio::time::timeout;

use crate::entrypoints::scheduler::state::SchedulerState;

pub mod fred;
pub mod jquants;
pub mod prediction;

pub(super) const DAILY_TIMEOUT: Duration = Duration::from_secs(60 * 60);
pub(super) const HOURLY_TIMEOUT: Duration = Duration::from_secs(50 * 60);
pub(super) const WEEKLY_TIMEOUT: Duration = Duration::from_secs(6 * 60 * 60);

pub(super) async fn run_with_state<F, Fut>(
    context: WorkerContext,
    task_name: &'static str,
    duration: Duration,
    task: F,
) -> Result<(), String>
where
    F: FnOnce(SchedulerState) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let Some(state) = context.get_ext::<SchedulerState>() else {
        return Err("scheduler state is not configured".to_string());
    };

    run_with_timeout(task_name, duration, task(state.clone())).await
}

pub(super) async fn run_jquants_job<F, Fut>(
    context: WorkerContext,
    task_name: &'static str,
    duration: Duration,
    task: F,
) -> Result<(), String>
where
    F: FnOnce(SchedulerState, Arc<JQuantsClient>) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    run_with_state(context, task_name, duration, |state| async move {
        let client = state
            .jquants_client
            .clone()
            .ok_or_else(|| "J-Quants client is not configured".to_string())?;
        task(state, client).await
    })
    .await
}

pub(super) async fn run_with_timeout<F>(
    task_name: &'static str,
    duration: Duration,
    task: F,
) -> Result<(), String>
where
    F: Future<Output = Result<(), String>>,
{
    match timeout(duration, task).await {
        Ok(Ok(())) => {
            tracing::info!(task = task_name, "scheduled job completed");
            Ok(())
        }
        Ok(Err(error)) => {
            tracing::warn!(task = task_name, %error, "scheduled job failed");
            Err(error)
        }
        Err(_) => {
            let error = format!("{task_name} timed out after {} seconds", duration.as_secs());
            tracing::error!(
                task = task_name,
                timeout_secs = duration.as_secs(),
                "scheduled job timed out"
            );
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::run_with_timeout;

    #[tokio::test]
    async fn returns_the_job_error_for_worker_retry() {
        assert_eq!(
            run_with_timeout("sample_task", std::time::Duration::from_secs(1), async {
                Err("operation failed".to_string())
            })
            .await,
            Err("operation failed".to_string()),
        );
    }

    #[tokio::test]
    async fn returns_an_error_when_the_job_exceeds_its_timeout() {
        assert_eq!(
            run_with_timeout(
                "sample_task",
                std::time::Duration::from_millis(1),
                std::future::pending(),
            )
            .await,
            Err("sample_task timed out after 0 seconds".to_string()),
        );
    }
}
