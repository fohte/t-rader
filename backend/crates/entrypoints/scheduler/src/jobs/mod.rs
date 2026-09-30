use std::future::Future;
use std::time::Duration;

use graphile_worker::WorkerContext;
use tokio::time::timeout;

use crate::state::SchedulerState;

pub mod fred;
pub mod jquants;
pub mod prediction;

pub(super) const DAILY_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
pub(super) const WEEKLY_TIMEOUT: Duration = Duration::from_secs(12 * 60 * 60);

pub(super) async fn run_with_state<F, Fut>(
    context: WorkerContext,
    task_name: &'static str,
    timeout: Duration,
    task: F,
) -> Result<(), String>
where
    F: FnOnce(SchedulerState) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let Some(state) = context.get_ext::<SchedulerState>() else {
        return Err("scheduler state is not configured".to_string());
    };

    run_with_timeout(task_name, timeout, task(state.clone())).await
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
            let error = format!("{task_name} timed out after {duration:?}");
            tracing::error!(task = task_name, ?duration, "scheduled job timed out");
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{future::pending, time::Duration};

    use rstest::rstest;

    use super::run_with_timeout;

    #[rstest]
    #[case::use_case_failure(Err("operation failed".to_string()), Err("operation failed".to_string()))]
    #[case::success(Ok(()), Ok(()))]
    #[tokio::test]
    async fn returns_the_job_result(
        #[case] task_result: Result<(), String>,
        #[case] expected: Result<(), String>,
    ) {
        assert_eq!(
            run_with_timeout("sample_task", Duration::from_secs(1), async { task_result }).await,
            expected,
        );
    }

    #[tokio::test]
    async fn returns_an_error_when_the_job_exceeds_its_timeout() {
        assert_eq!(
            run_with_timeout("sample_task", Duration::from_millis(1), pending()).await,
            Err("sample_task timed out after 1ms".to_string()),
        );
    }
}
