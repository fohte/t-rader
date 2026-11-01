use chrono::Utc;
use chrono_tz::Asia::Tokyo;
use core_application::calendar::error::CalendarEventUseCaseError;
use core_application::calendar::source::CalendarEventSource;
use core_application::calendar::use_cases::{CalendarEventIngestStats, CalendarEventUseCases};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

pub struct EStatCalendarIngest;

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
        .map_err(|error: CalendarEventUseCaseError| error.to_string())?;
    tracing::debug!(
        upserted = stats.upserted,
        deleted = stats.deleted,
        "e-Stat calendar ingest cycle completed"
    );
    if let Some(macro_references) = macro_references {
        let references = macro_references
            .fetch_pending(today, "e_stat")
            .await
            .map_err(|error| error.to_string())?;
        tracing::debug!(
            events_checked = references.events_checked,
            references_upserted = references.references_upserted,
            external_failures = references.external_failures,
            budget_exhausted = references.budget_exhausted,
            "e-Stat macro references checked"
        );
    }
    Ok(stats)
}
