use core_application::ingest_status::BOJ_CALENDAR_EVENT_INGEST_JOB;
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{
    DAILY_TIMEOUT, calendar_event::ingest_calendar_events, require_source,
    run_with_ingest_run_log_state,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct BojCalendarEventIngest;

impl TaskHandler for BojCalendarEventIngest {
    const IDENTIFIER: &'static str = BOJ_CALENDAR_EVENT_INGEST_JOB;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = require_source(
                    state.dependencies.boj_calendar_source,
                    "BOJ calendar event source",
                )?;
                ingest_calendar_events(&state.dependencies.calendar_events, source.as_ref()).await
            },
        )
        .await
    }
}
