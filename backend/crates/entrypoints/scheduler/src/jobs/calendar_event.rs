use chrono::Utc;
use chrono_tz::Asia::Tokyo;
use core_application::calendar::{
    source::CalendarEventSource,
    use_cases::{CalendarEventIngestStats, CalendarEventUseCases},
};

pub(super) async fn ingest_calendar_events(
    use_cases: &CalendarEventUseCases,
    source: &dyn CalendarEventSource,
) -> Result<CalendarEventIngestStats, String> {
    let today = Utc::now().with_timezone(&Tokyo).date_naive();
    let stats = use_cases
        .run_ingest_cycle(source, today)
        .await
        .map_err(|error| error.to_string())?;
    tracing::debug!(
        upserted = stats.upserted,
        deleted = stats.deleted,
        source = source.source(),
        "calendar event ingest cycle completed"
    );
    Ok(stats)
}
