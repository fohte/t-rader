use chrono::Utc;
use core_application::calendar::use_cases::CalendarEventIngestStats;
use core_application::{
    calendar::{source::CalendarEventSource, use_cases::CalendarEventUseCases},
    ingest_status::FRED_RELEASE_DATES_INGEST_JOB,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct FredReleaseDatesIngest;

impl TaskHandler for FredReleaseDatesIngest {
    const IDENTIFIER: &'static str = FRED_RELEASE_DATES_INGEST_JOB;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = require_source(
                    state.dependencies.fred_calendar_event_source,
                    "FRED release dates source",
                )?;
                ingest_calendar_events(&state.dependencies.calendar_events, source.as_ref()).await
            },
        )
        .await
    }
}

async fn ingest_calendar_events(
    use_cases: &CalendarEventUseCases,
    source: &dyn CalendarEventSource,
) -> Result<CalendarEventIngestStats, String> {
    let stats = use_cases
        .run_ingest_cycle(source, Utc::now().date_naive())
        .await
        .map_err(|error| error.to_string())?;
    tracing::debug!(
        upserted = stats.upserted,
        deleted = stats.deleted,
        "FRED release dates ingest cycle completed"
    );
    Ok(stats)
}
