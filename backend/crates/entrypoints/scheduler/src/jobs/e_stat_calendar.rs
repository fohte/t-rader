use chrono::Utc;
use core_application::calendar::error::CalendarEventUseCaseError;
use core_application::calendar::source::CalendarEventSource;
use core_application::calendar::use_cases::{CalendarEventIngestStats, CalendarEventUseCases};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct EStatCalendarIngest;

impl TaskHandler for EStatCalendarIngest {
    const IDENTIFIER: &'static str = <Self as super::SchedulerJob>::IDENTIFIER;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        super::run_from_worker_context(context, <Self as super::SchedulerJob>::run).await
    }
}

impl super::SchedulerJob for EStatCalendarIngest {
    const IDENTIFIER: &'static str = core_application::ingest_status::E_STAT_CALENDAR_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let source = require_source(
                        state.dependencies.e_stat_calendar_source,
                        "e-Stat calendar source",
                    )?;
                    ingest_calendar_events(&state.dependencies.calendar_events, source.as_ref())
                        .await
                },
            )
            .await
        })
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
        "e-Stat calendar ingest cycle completed"
    );
    Ok(stats)
}
