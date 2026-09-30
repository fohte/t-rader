use std::future::Future;
use std::time::Duration;

use tokio::time::timeout;

pub mod fred;
pub mod jquants;
pub mod prediction;

pub(super) const DAILY_TIMEOUT: Duration = Duration::from_secs(60 * 60);
pub(super) const HOURLY_TIMEOUT: Duration = Duration::from_secs(50 * 60);
pub(super) const WEEKLY_TIMEOUT: Duration = Duration::from_secs(6 * 60 * 60);

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
