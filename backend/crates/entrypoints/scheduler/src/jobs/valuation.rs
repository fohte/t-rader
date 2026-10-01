use chrono::Utc;
use core_application::{
    valuation::{IngestStats as ValuationIngestStats, ValuationUseCaseError, ValuationUseCases},
    valuation_source::ValuationSource,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct ValuationIngest;

impl TaskHandler for ValuationIngest {
    const IDENTIFIER: &'static str = "valuation_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state(
            context,
            Self::IDENTIFIER,
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
    }
}

async fn ingest_valuations(
    use_cases: &ValuationUseCases,
    source: &dyn ValuationSource,
) -> Result<ValuationIngestStats, String> {
    let stats = use_cases
        .run_ingest_cycle(source, Utc::now().date_naive())
        .await
        .map_err(|error: ValuationUseCaseError| error.to_string())?;
    tracing::debug!(
        days_attempted = stats.days_attempted,
        rows_upserted = stats.rows_upserted,
        "valuation ingest cycle completed"
    );
    Ok(stats)
}
