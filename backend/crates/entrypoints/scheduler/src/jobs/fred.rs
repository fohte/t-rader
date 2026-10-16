use core_application::indicator_observation::IndicatorObservationIngestSeriesResult;

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

pub struct FredIngest;

impl super::SchedulerJob for FredIngest {
    const IDENTIFIER: &'static str = core_application::ingest_status::FRED_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                        state,
                        <Self as super::SchedulerJob>::IDENTIFIER,
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
        })
    }
}
