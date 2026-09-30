use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, run_with_timeout};
use crate::entrypoints::scheduler::state::SchedulerState;
use core_application::indicator_observation::IndicatorObservationIngestSeriesResult;

#[derive(Debug, Deserialize, Serialize)]
pub struct FredIngest;

impl TaskHandler for FredIngest {
    const IDENTIFIER: &'static str = "fred_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        let Some(state) = context.get_ext::<SchedulerState>() else {
            return Err("scheduler state is not configured".to_string());
        };

        run_with_timeout(Self::IDENTIFIER, DAILY_TIMEOUT, async {
            let source = state
                .fred_source
                .as_deref()
                .ok_or_else(|| "FRED source is not configured".to_string())?;
            let result = state
                .use_cases
                .indicator_observations()
                .ingest(source)
                .await;
            let failures = result
                .series
                .into_iter()
                .filter_map(|outcome| match outcome {
                    IndicatorObservationIngestSeriesResult::Succeeded {
                        series_id,
                        upserted,
                    } => {
                        tracing::debug!(%series_id, upserted, "FRED series ingest completed");
                        None
                    }
                    IndicatorObservationIngestSeriesResult::Failed { series_id, error } => {
                        tracing::warn!(%series_id, %error, "FRED series ingest failed");
                        Some(format!("{series_id}: {error}"))
                    }
                })
                .collect::<Vec<_>>();
            if failures.is_empty() {
                Ok(())
            } else {
                Err(failures.join("; "))
            }
        })
        .await
    }
}
