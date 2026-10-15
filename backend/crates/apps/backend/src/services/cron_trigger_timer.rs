use std::time::Duration;

use chrono::Utc;
use core_application::agent_task_client::SharedAgentTaskClient;
use core_application::trigger::TriggerUseCases;
use gateway_postgres::PostgresTriggerChangeListener;
use tokio::sync::watch;
use tokio::time::{Instant, sleep_until};

const RECOVERY_WINDOW: Duration = Duration::from_secs(60);
const ERROR_RETRY_INTERVAL: Duration = Duration::from_secs(30);

pub async fn run(
    triggers: TriggerUseCases,
    agent_task_client: SharedAgentTaskClient,
    mut listener: PostgresTriggerChangeListener,
    shutdown: watch::Receiver<bool>,
) {
    loop {
        triggers
            .run_cron_tick(agent_task_client.as_ref(), RECOVERY_WINDOW)
            .await;
        let now = Utc::now();
        let next_fire_at = match triggers.next_cron_fire_at(now, RECOVERY_WINDOW).await {
            Ok(next_fire_at) => next_fire_at,
            Err(error) => {
                tracing::warn!(error = %error, "failed to schedule next cron trigger");
                Some(now + chrono::Duration::from_std(ERROR_RETRY_INTERVAL).unwrap_or_default())
            }
        };
        let next_fire_at = next_fire_at.map(|next_fire_at| {
            if next_fire_at <= now {
                now + chrono::Duration::from_std(ERROR_RETRY_INTERVAL).unwrap_or_default()
            } else {
                next_fire_at
            }
        });

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
            _ = wait_for_shutdown(shutdown.clone()) => return,
            result = listener.recv() => {
                if let Err(error) = result {
                    tracing::warn!(error = %error, "failed to receive trigger change notification");
                    tokio::select! {
                        _ = wait_for_shutdown(shutdown.clone()) => return,
                        _ = tokio::time::sleep(ERROR_RETRY_INTERVAL) => {}
                    }
                }
            }
            _ = wait_for_timer => {}
        }
    }
}

async fn wait_for_shutdown(mut shutdown: watch::Receiver<bool>) {
    if *shutdown.borrow() {
        return;
    }
    let _ = shutdown.wait_for(|is_shutdown| *is_shutdown).await;
}
