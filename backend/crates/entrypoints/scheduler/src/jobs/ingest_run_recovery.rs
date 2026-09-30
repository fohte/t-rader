use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use core_application::ingest_run_log::IngestRunLog;
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use crate::state::SchedulerState;

use super::{
    DAILY_TIMEOUT, WEEKLY_TIMEOUT,
    fred::FredIngest,
    jquants::{MarginIngest, ShortRatioIngest, ShortSaleReportIngest},
    prediction::PredictionGrading,
};

const RECOVERABLE_JOBS: [(&str, Duration); 5] = [
    (FredIngest::IDENTIFIER, DAILY_TIMEOUT),
    (ShortRatioIngest::IDENTIFIER, DAILY_TIMEOUT),
    (ShortSaleReportIngest::IDENTIFIER, DAILY_TIMEOUT),
    (MarginIngest::IDENTIFIER, DAILY_TIMEOUT),
    (PredictionGrading::IDENTIFIER, WEEKLY_TIMEOUT),
];

#[derive(Debug, Deserialize, Serialize)]
pub struct IngestRunRecovery;

impl TaskHandler for IngestRunRecovery {
    const IDENTIFIER: &'static str = "ingest_run_recovery";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        let Some(state) = context.get_ext::<SchedulerState>() else {
            return Err("scheduler state is not configured".to_string());
        };

        recover_interrupted_runs(state.dependencies.ingest_run_log.as_ref()).await
    }
}

async fn recover_interrupted_runs(log: &dyn IngestRunLog) -> Result<(), String> {
    let now = Utc::now().fixed_offset();
    let mut recovered = 0_u64;

    for (job, timeout) in RECOVERABLE_JOBS {
        let cutoff = now - ChronoDuration::seconds(timeout.as_secs() as i64);
        recovered = recovered.saturating_add(
            log.fail_interrupted_before(job, cutoff)
                .await
                .map_err(|error| error.to_string())?,
        );
    }

    if recovered > 0 {
        tracing::info!(recovered, "interrupted ingest runs recovered");
    }
    Ok(())
}
