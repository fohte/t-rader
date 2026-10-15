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
    const IDENTIFIER: &'static str = <Self as super::SchedulerJob>::IDENTIFIER;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        super::run_from_worker_context(context, <Self as super::SchedulerJob>::run).await
    }
}

impl super::SchedulerJob for EodDataKospiIngest {
    const IDENTIFIER: &'static str = core_application::ingest_status::EODDATA_KOSPI_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let source = require_source(
                        state.dependencies.eoddata_kospi_source,
                        "EODData KOSPI source",
                    )?;
                    let upserted = state
                        .dependencies
                        .indicator_observations
                        .ingest_eoddata_series(source.as_ref())
                        .await?;

                    Ok(EodDataKospiIngestStats { upserted })
                },
            )
            .await
        })
    }
}
