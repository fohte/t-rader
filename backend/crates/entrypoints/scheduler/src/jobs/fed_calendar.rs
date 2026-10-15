use core_application::ingest_status::FED_CALENDAR_EVENT_INGEST_JOB;
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{
    DAILY_TIMEOUT, calendar_event::ingest_calendar_events, require_source,
    run_with_ingest_run_log_state,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct FedCalendarEventIngest;

impl TaskHandler for FedCalendarEventIngest {
    const IDENTIFIER: &'static str = <Self as super::SchedulerJob>::IDENTIFIER;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        super::run_from_worker_context(context, <Self as super::SchedulerJob>::run).await
    }
}

impl super::SchedulerJob for FedCalendarEventIngest {
    const IDENTIFIER: &'static str = FED_CALENDAR_EVENT_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let source = require_source(
                        state.dependencies.fed_calendar_source,
                        "Federal Reserve calendar event source",
                    )?;
                    ingest_calendar_events(&state.dependencies.calendar_events, source.as_ref())
                        .await
                },
            )
            .await
        })
    }
}
