use chrono::Utc;
use chrono_tz::Asia::Taipei;
use core_application::{
    indicator_observation::TWSE_SERIES,
    indicator_observation::{
        IndicatorObservationIngestResult, IndicatorObservationIngestSeriesResult,
    },
    ingest_status::TWSE_INDEX_INGEST_JOB,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, run_with_ingest_run_log_state_and_failure_stats};

#[derive(Debug, Deserialize, Serialize)]
pub struct TwseIndexIngest;

impl TaskHandler for TwseIndexIngest {
    const IDENTIFIER: &'static str = TWSE_INDEX_INGEST_JOB;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state_and_failure_stats(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let Some(source) = state.dependencies.twse_source else {
                    return Err((
                        IndicatorObservationIngestResult {
                            series: Vec::new(),
                            errors: Vec::new(),
                        },
                        "TWSE source is not configured".to_string(),
                    ));
                };
                let today = Utc::now().with_timezone(&Taipei).date_naive();
                let result = state
                    .dependencies
                    .indicator_observations
                    .ingest_batch(source.as_ref(), TWSE_SERIES, None, today)
                    .await;
                let mut failures: Vec<_> = result
                    .errors
                    .iter()
                    .map(|error| format!("{}: {}", error.date, error.message))
                    .collect();
                failures.extend(
                    result
                        .series
                        .iter()
                        .filter_map(|outcome| match outcome {
                            IndicatorObservationIngestSeriesResult::Succeeded { .. } => None,
                            IndicatorObservationIngestSeriesResult::Failed { series_id, error } => {
                                Some(format!("{series_id}: {error}"))
                            }
                        })
                        .collect::<Vec<_>>(),
                );
                if failures.is_empty() {
                    Ok(result)
                } else {
                    Err((result, failures.join("; ")))
                }
            },
        )
        .await
    }
}
