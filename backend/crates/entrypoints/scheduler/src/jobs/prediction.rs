use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{WEEKLY_TIMEOUT, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct PredictionGrading;

impl TaskHandler for PredictionGrading {
    const IDENTIFIER: &'static str = <Self as super::SchedulerJob>::IDENTIFIER;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        super::run_from_worker_context(context, <Self as super::SchedulerJob>::run).await
    }
}

impl super::SchedulerJob for PredictionGrading {
    const IDENTIFIER: &'static str = core_application::ingest_status::PREDICTION_GRADING_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                WEEKLY_TIMEOUT,
                |state| async move {
                    let stats = state
                        .dependencies
                        .predictions
                        .grade_due_today()
                        .await
                        .map_err(|error| error.to_string())?;
                    tracing::debug!(graded = stats.graded, "prediction grading completed");
                    Ok(stats)
                },
            )
            .await
        })
    }
}
