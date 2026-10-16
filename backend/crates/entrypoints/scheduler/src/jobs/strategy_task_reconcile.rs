use core_application::strategy_task::STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER;

use super::{DAILY_TIMEOUT, run_with_state};

pub struct StrategyTaskReconcile;

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
                    let dispatch_result = dispatch_pending_events(&state)
                        .await
                        .map_err(|error| format!("failed to dispatch pending events: {error}"));

                    combine_task_results(reconcile_result, dispatch_result)
                },
            )
            .await
        })
    }
}

async fn dispatch_pending_events(state: &crate::state::SchedulerState) -> Result<(), String> {
    let (dispatched, deleted) = state
        .dependencies
        .event_dispatch
        .dispatch_and_cleanup(state.dependencies.agent_task_client.as_ref())
        .await
        .map_err(|error| error.to_string())?;
    if dispatched > 0 {
        tracing::info!(dispatched, "pending events dispatched");
    }
    if deleted > 0 {
        tracing::info!(deleted, "expired events deleted");
    }
    Ok(())
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
