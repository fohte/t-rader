use core_application::strategy_task::STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER;
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, run_with_state};

#[derive(Debug, Deserialize, Serialize)]
#[serde(from = "Option<EmptyObject>")]
pub struct StrategyTaskReconcile;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyObject {}

impl From<Option<EmptyObject>> for StrategyTaskReconcile {
    fn from(_payload: Option<EmptyObject>) -> Self {
        Self
    }
}

impl TaskHandler for StrategyTaskReconcile {
    const IDENTIFIER: &'static str = <Self as super::SchedulerJob>::IDENTIFIER;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        super::run_from_worker_context(context, <Self as super::SchedulerJob>::run).await
    }
}

impl super::SchedulerJob for StrategyTaskReconcile {
    const IDENTIFIER: &'static str = STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let reconcile_result = state
                        .dependencies
                        .strategy_task_reconcile
                        .reconcile()
                        .await
                        .map(|updated| {
                            if updated > 0 {
                                tracing::info!(updated, "strategy task phases reconciled");
                            }
                        });
                    let dispatch_result = super::event_dispatch::dispatch_pending_events(&state)
                        .await
                        .map_err(|error| format!("failed to dispatch pending events: {error}"));

                    combine_task_results(reconcile_result, dispatch_result)
                },
            )
            .await
        })
    }
}

fn combine_task_results(
    first_result: Result<(), String>,
    second_result: Result<(), String>,
) -> Result<(), String> {
    match (first_result, second_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(first_error), Err(second_error)) => Err(format!("{first_error}; {second_error}")),
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::StrategyTaskReconcile;

    #[rstest]
    #[case::cron_payload_null(serde_json::Value::Null, Ok(()))]
    #[case::cron_payload_empty_object(serde_json::json!({}), Ok(()))]
    #[case::unexpected_field_is_rejected(serde_json::json!({"unexpected": true}), Err(()))]
    fn deserializes_reconcile_job_payloads(
        #[case] payload: serde_json::Value,
        #[case] expected: Result<(), ()>,
    ) {
        assert_eq!(
            serde_json::from_value::<StrategyTaskReconcile>(payload)
                .map(|_| ())
                .map_err(|_| ()),
            expected,
        );
    }
}
