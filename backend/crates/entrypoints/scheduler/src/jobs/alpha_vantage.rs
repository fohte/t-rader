use chrono::Utc;
use core_application::calendar::{
    error::CalendarEventUseCaseError,
    source::CalendarEventSource,
    use_cases::{CalendarEventIngestStats, CalendarEventUseCases},
};
use core_application::sec_filing_reference::{SecFilingReferenceStats, SecFilingReferenceUseCases};
use core_application::sec_filing_source::SecFilingSource;
use serde::Serialize;
use std::sync::Arc;

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state, today_in_tokyo};

#[derive(Serialize)]
struct AlphaVantageCalendarIngestStats {
    calendar_events: CalendarEventIngestStats,
    sec_filing_references: Option<SecFilingReferenceStats>,
}

pub struct AlphaVantageCalendarIngest;

impl super::SchedulerJob for AlphaVantageCalendarIngest {
    const IDENTIFIER: &'static str =
        core_application::ingest_status::ALPHA_VANTAGE_CALENDAR_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let source = require_source(
                        state.dependencies.alpha_vantage_calendar_source,
                        "Alpha Vantage calendar source",
                    )?;
                    ingest_calendar_events(
                        &state.dependencies.calendar_events,
                        source.as_ref(),
                        &state.dependencies.sec_filing_references,
                        state
                            .dependencies
                            .sec_filing_source
                            .as_ref()
                            .map(Arc::as_ref),
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
    sec_filing_references: &SecFilingReferenceUseCases,
    sec_filing_source: Option<&dyn SecFilingSource>,
) -> Result<AlphaVantageCalendarIngestStats, String> {
    let calendar_stats = use_cases
        .run_ingest_cycle(source, Utc::now().date_naive())
        .await
        .map_err(|error: CalendarEventUseCaseError| error.to_string())?;
    tracing::debug!(
        upserted = calendar_stats.upserted,
        deleted = calendar_stats.deleted,
        "Alpha Vantage calendar ingest cycle completed"
    );
    let sec_stats = if let Some(source) = sec_filing_source {
        let stats = sec_filing_references
            .fetch_pending(source, today_in_tokyo(Utc::now()))
            .await
            .map_err(|error| error.to_string())?;
        tracing::debug!(
            events_checked = stats.events_checked,
            companies_checked = stats.companies_checked,
            external_failures = stats.external_failures,
            references_upserted = stats.references_upserted,
            budget_exhausted = stats.budget_exhausted,
            "SEC filing reference fetch completed"
        );
        Some(stats)
    } else {
        None
    };
    Ok(AlphaVantageCalendarIngestStats {
        calendar_events: calendar_stats,
        sec_filing_references: sec_stats,
    })
}
