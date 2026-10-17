use serde::Serialize;

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

#[derive(Debug, Serialize)]
struct EodDataKospiIngestStats {
    upserted: usize,
}

pub struct EodDataKospiIngest;

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
