use chrono::Utc;
use core_application::{
    valuation::{IngestStats as ValuationIngestStats, ValuationUseCaseError, ValuationUseCases},
    valuation_source::ValuationSource,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state, today_in_tokyo};

#[derive(Debug, Deserialize, Serialize)]
pub struct ValuationIngest;

impl TaskHandler for ValuationIngest {
    const IDENTIFIER: &'static str = <Self as super::SchedulerJob>::IDENTIFIER;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        super::run_from_worker_context(context, <Self as super::SchedulerJob>::run).await
    }
}

impl super::SchedulerJob for ValuationIngest {
    const IDENTIFIER: &'static str = core_application::ingest_status::VALUATION_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let source = require_source(
                        state.dependencies.valuation_source,
                        "J-Quants valuation source",
                    )?;
                    ingest_valuations(&state.dependencies.valuations, source.as_ref()).await
                },
            )
            .await
        })
    }
}

async fn ingest_valuations(
    use_cases: &ValuationUseCases,
    source: &dyn ValuationSource,
) -> Result<ValuationIngestStats, String> {
    let stats = use_cases
        .run_ingest_cycle(source, today_in_tokyo(Utc::now()))
        .await
        .map_err(|error: ValuationUseCaseError| error.to_string())?;
    tracing::debug!(
        days_attempted = stats.days_attempted,
        rows_upserted = stats.rows_upserted,
        "valuation ingest cycle completed"
    );
    Ok(stats)
}
