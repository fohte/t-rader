use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{DAILY_TIMEOUT, run_with_state};

const TRIGGER_EVALUATION_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Deserialize, Serialize)]
pub struct TriggerEvaluation;

impl TaskHandler for TriggerEvaluation {
    const IDENTIFIER: &'static str = <Self as super::SchedulerJob>::IDENTIFIER;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        super::run_from_worker_context(context, <Self as super::SchedulerJob>::run).await
    }
}

impl super::SchedulerJob for TriggerEvaluation {
    const IDENTIFIER: &'static str = "trigger_evaluation";

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let attempts = state
                        .dependencies
                        .triggers
                        .run_cron_tick(
                            state.dependencies.agent_task_client.as_ref(),
                            TRIGGER_EVALUATION_INTERVAL,
                        )
                        .await;
                    if attempts > 0 {
                        tracing::info!(attempts, "cron triggers evaluated");
                    }
                    Ok(())
                },
            )
            .await
        })
    }
}
