use chrono::Utc;
use core_application::{
    earnings_schedule::{
        EarningsScheduleIngestStats, EarningsScheduleUseCaseError, EarningsScheduleUseCases,
    },
    earnings_schedule_source::EarningsScheduleSource,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct EarningsScheduleIngest;

impl TaskHandler for EarningsScheduleIngest {
    const IDENTIFIER: &'static str = "earnings_schedule_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = require_source(
                    state.dependencies.earnings_schedule_source,
                    "J-Quants earnings schedule source",
                )?;
                ingest_earnings_schedules(&state.dependencies.earnings_schedules, source.as_ref())
                    .await
            },
        )
        .await
    }
}

async fn ingest_earnings_schedules(
    use_cases: &EarningsScheduleUseCases,
    source: &dyn EarningsScheduleSource,
) -> Result<EarningsScheduleIngestStats, String> {
    let stats = use_cases
        .run_ingest_cycle(source, Utc::now().date_naive())
        .await
        .map_err(|error: EarningsScheduleUseCaseError| error.to_string())?;
    tracing::debug!(
        days_attempted = stats.days_attempted,
        upserted = stats.upserted,
        "earnings schedule ingest cycle completed"
    );
    Ok(stats)
}
