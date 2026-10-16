use core_application::ingest_status::BOJ_CALENDAR_EVENT_INGEST_JOB;

use super::{
    DAILY_TIMEOUT, calendar_event::ingest_calendar_events, require_source,
    run_with_ingest_run_log_state,
};

pub struct BojCalendarEventIngest;

impl super::SchedulerJob for BojCalendarEventIngest {
    const IDENTIFIER: &'static str = BOJ_CALENDAR_EVENT_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let source = require_source(
                        state.dependencies.boj_calendar_source,
                        "BOJ calendar event source",
                    )?;
                    ingest_calendar_events(&state.dependencies.calendar_events, source.as_ref())
                        .await
                },
            )
            .await
        })
    }
}
