use core_application::indicator_observation::IndicatorObservationMetadata;
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::Serialize;

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

#[derive(Debug, Serialize)]
struct EodDataKospiIngestStats {
    upserted: usize,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct EodDataKospiIngest;

impl TaskHandler for EodDataKospiIngest {
    const IDENTIFIER: &'static str = core_application::ingest_status::EODDATA_KOSPI_INGEST_JOB;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = require_source(
                    state.dependencies.eoddata_kospi_source,
                    "EODData KOSPI source",
                )?;
                let upserted = state
                    .dependencies
                    .indicator_observations
                    .ingest_single_series(
                        source.as_ref(),
                        "KSIC",
                        IndicatorObservationMetadata {
                            indicator_id: "KOSPI".to_string(),
                            name: "韓国総合株価指数".to_string(),
                            kind: "index".to_string(),
                        },
                    )
                    .await?;

                Ok(EodDataKospiIngestStats { upserted })
            },
        )
        .await
    }
}
