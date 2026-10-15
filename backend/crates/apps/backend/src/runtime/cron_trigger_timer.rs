use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use core_application::agent_task_client::SharedAgentTaskClient;
use core_application::trigger::TriggerUseCases;
use gateway_postgres::PostgresTriggerChangeListener;
use tokio::sync::watch;
use tokio::time::{Instant, sleep_until};

const RECOVERY_WINDOW: Duration = Duration::from_secs(60);
const ERROR_RETRY_INTERVAL: Duration = Duration::from_secs(30);

#[async_trait]
pub(crate) trait TriggerChangeListener: Send {
    async fn recv(&mut self) -> Result<(), String>;
}

#[async_trait]
impl TriggerChangeListener for PostgresTriggerChangeListener {
    async fn recv(&mut self) -> Result<(), String> {
        PostgresTriggerChangeListener::recv(self)
            .await
            .map_err(|error| error.to_string())
    }
}

pub(crate) async fn run<L>(
    triggers: TriggerUseCases,
    agent_task_client: SharedAgentTaskClient,
    mut listener: L,
    shutdown: watch::Receiver<bool>,
) where
    L: TriggerChangeListener + 'static,
{
    loop {
        let attempts = triggers
            .run_cron_tick(agent_task_client.as_ref(), RECOVERY_WINDOW)
            .await;
        if attempts > 0 {
            tracing::info!(attempts, "cron triggers evaluated");
        }
        let now = Utc::now();
        let next_fire_at = match triggers
            .next_cron_fire_at(now, RECOVERY_WINDOW, ERROR_RETRY_INTERVAL)
            .await
        {
            Ok(next_fire_at) => next_fire_at,
            Err(error) => {
                tracing::warn!(error = %error, "failed to schedule next cron trigger");
                Some(retry_at(now))
            }
        };
        let next_fire_at = next_wake_at(next_fire_at, now);

        let wait_for_timer = async {
            if let Some(next_fire_at) = next_fire_at {
                let delay = next_fire_at
                    .signed_duration_since(Utc::now())
                    .to_std()
                    .unwrap_or_default();
                sleep_until(Instant::now() + delay).await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        tokio::select! {
            _ = crate::signals::wait_for_shutdown(shutdown.clone()) => return,
            result = listener.recv() => {
                if let Err(error) = result {
                    tracing::warn!(error, "failed to receive trigger change notification");
                    tokio::select! {
                        _ = crate::signals::wait_for_shutdown(shutdown.clone()) => return,
                        _ = tokio::time::sleep(ERROR_RETRY_INTERVAL) => {}
                    }
                }
            }
            _ = wait_for_timer => {}
        }
    }
}

fn next_wake_at(next_fire_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    next_fire_at.map(|next_fire_at| {
        if next_fire_at <= now {
            retry_at(now)
        } else {
            next_fire_at
        }
    })
}

fn retry_at(now: DateTime<Utc>) -> DateTime<Utc> {
    now + chrono::Duration::from_std(ERROR_RETRY_INTERVAL).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use rstest::rstest;

    use super::next_wake_at;

    fn timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid test timestamp")
            .with_timezone(&Utc)
    }

    #[rstest]
    #[case::missing_deadline(None, None)]
    #[case::future_deadline(Some("2026-01-02T12:35:00Z"), Some("2026-01-02T12:35:00Z"))]
    #[case::overdue_deadline(Some("2026-01-02T12:34:00Z"), Some("2026-01-02T12:34:30Z"))]
    fn chooses_a_wake_time_without_a_busy_loop(
        #[case] next_fire_at: Option<&str>,
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(
            next_wake_at(
                next_fire_at.map(timestamp),
                timestamp("2026-01-02T12:34:00Z")
            ),
            expected.map(timestamp),
        );
    }
}
