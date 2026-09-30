use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{WEEKLY_TIMEOUT, run_with_timeout};
use crate::entrypoints::scheduler::state::SchedulerState;

#[derive(Debug, Deserialize, Serialize)]
pub struct PredictionGrading;

impl TaskHandler for PredictionGrading {
    const IDENTIFIER: &'static str = "prediction_grading";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        let Some(state) = context.get_ext::<SchedulerState>() else {
            return Err("scheduler state is not configured".to_string());
        };

        run_with_timeout(Self::IDENTIFIER, WEEKLY_TIMEOUT, async {
            let stats =
                crate::services::prediction_grading::run_once(&state.use_cases.predictions())
                    .await
                    .map_err(|error| error.to_string())?;
            tracing::debug!(graded = stats.graded, "prediction grading completed");
            Ok(())
        })
        .await
    }
}
