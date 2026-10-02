use core_application::indicator_observation::IndicatorObservationIngestSeriesResult;
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct FredIngest;

impl TaskHandler for FredIngest {
    const IDENTIFIER: &'static str = core_application::ingest_status::FRED_INGEST_JOB;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = require_source(state.dependencies.fred_source, "FRED source")?;

                let result = state
                    .dependencies
                    .indicator_observations
                    .ingest(source.as_ref())
                    .await;
                for outcome in &result.series {
                    match outcome {
                        IndicatorObservationIngestSeriesResult::Succeeded {
                            series_id,
                            upserted,
                        } => tracing::debug!(%series_id, upserted, "FRED series ingest completed"),
                        IndicatorObservationIngestSeriesResult::Failed { series_id, error } => {
                            tracing::warn!(%series_id, %error, "FRED series ingest failed");
                        }
                    }
                }
                Ok(result)
            },
        )
        .await
    }
}
