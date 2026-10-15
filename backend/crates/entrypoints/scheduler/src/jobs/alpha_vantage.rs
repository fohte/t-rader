use chrono::Utc;
use core_application::calendar::{
    error::CalendarEventUseCaseError,
    source::CalendarEventSource,
    use_cases::{CalendarEventIngestStats, CalendarEventUseCases},
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct AlphaVantageCalendarIngest;

impl TaskHandler for AlphaVantageCalendarIngest {
    const IDENTIFIER: &'static str =
        core_application::ingest_status::ALPHA_VANTAGE_CALENDAR_INGEST_JOB;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = require_source(
                    state.dependencies.alpha_vantage_calendar_source,
                    "Alpha Vantage calendar source",
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
        .map_err(|error: CalendarEventUseCaseError| error.to_string())?;
    tracing::debug!(
        upserted = stats.upserted,
        deleted = stats.deleted,
        "Alpha Vantage calendar ingest cycle completed"
    );
    Ok(stats)
}
