use core_application::ingest_status::ECB_CALENDAR_EVENT_INGEST_JOB;
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{
    DAILY_TIMEOUT, calendar_event::ingest_calendar_events, require_source,
    run_with_ingest_run_log_state,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct EcbCalendarEventIngest;

impl TaskHandler for EcbCalendarEventIngest {
    const IDENTIFIER: &'static str = ECB_CALENDAR_EVENT_INGEST_JOB;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = require_source(
                    state.dependencies.ecb_calendar_source,
                    "ECB calendar event source",
                )?;
                ingest_calendar_events(&state.dependencies.calendar_events, source.as_ref()).await
            },
        )
        .await
    }
}
