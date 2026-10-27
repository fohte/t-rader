use chrono::Utc;
use chrono_tz::Asia::Tokyo;
use core_application::calendar::use_cases::CalendarEventIngestStats;
use core_application::{
    calendar::{source::CalendarEventSource, use_cases::CalendarEventUseCases},
    ingest_status::FRED_RELEASE_DATES_INGEST_JOB,
};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

pub struct FredReleaseDatesIngest;

impl super::SchedulerJob for FredReleaseDatesIngest {
    const IDENTIFIER: &'static str = FRED_RELEASE_DATES_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let source = require_source(
                        state.dependencies.fred_calendar_event_source,
                        "FRED release dates source",
                    )?;
                    ingest_calendar_events(
                        &state.dependencies.calendar_events,
                        source.as_ref(),
                        state.dependencies.macro_references.as_ref(),
                    )
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
    macro_references: Option<&core_application::macro_reference::MacroReferenceUseCases>,
) -> Result<CalendarEventIngestStats, String> {
    let today = Utc::now().with_timezone(&Tokyo).date_naive();
    let stats = use_cases
        .run_ingest_cycle(source, today)
        .await
        .map_err(|error| error.to_string())?;
    tracing::debug!(
        upserted = stats.upserted,
        deleted = stats.deleted,
        "FRED release dates ingest cycle completed"
    );
    if let Some(macro_references) = macro_references {
        let references = macro_references
            .fetch_pending(today, "fred")
            .await
            .map_err(|error| error.to_string())?;
        tracing::debug!(
            events_checked = references.events_checked,
            references_upserted = references.references_upserted,
            external_failures = references.external_failures,
            budget_exhausted = references.budget_exhausted,
            "FRED macro references checked"
        );
    }
    Ok(stats)
}
