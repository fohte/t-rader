use core_application::indicator_observation::IndicatorObservationIngestSeriesResult;
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, run_with_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct FredIngest;

impl TaskHandler for FredIngest {
    const IDENTIFIER: &'static str = "fred_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let Some(source) = state.dependencies.fred_source else {
                    return Err("FRED source is not configured".to_string());
                };

                let result = state
                    .dependencies
                    .indicator_observations
                    .ingest(source.as_ref())
                    .await;
                for outcome in result.series {
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
                Ok(())
            },
        )
        .await
    }
}
