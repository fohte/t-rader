use chrono::Utc;
use chrono_tz::Asia::Tokyo;
use core_application::calendar::{
    source::CalendarEventSource,
    use_cases::{CalendarEventIngestStats, CalendarEventUseCases},
};
use core_application::macro_reference::MacroReferenceUseCases;

pub(super) async fn ingest_calendar_events_with_macro_references(
    use_cases: &CalendarEventUseCases,
    source: &dyn CalendarEventSource,
    macro_references: Option<&MacroReferenceUseCases>,
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
    if let Some(macro_references) = macro_references {
        let references = macro_references
            .fetch_pending(today, source.source())
            .await
            .map_err(|error| error.to_string())?;
        tracing::debug!(
            events_checked = references.events_checked,
            references_upserted = references.references_upserted,
            external_failures = references.external_failures,
            budget_exhausted = references.budget_exhausted,
            event_source = source.source(),
            "calendar event references checked"
        );
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{DateTime, NaiveDate};
    use core_application::{
        calendar::{
            macro_reference_targets::{
                MacroReferenceTarget, MacroReferenceTargetRepository, MacroReferenceTargetSelector,
            },
            references::CalendarEventReferenceUseCases,
            repository::{
                CalendarEventRepository, CalendarEventRepositoryError,
                SharedCalendarEventRepository,
            },
            source::{CalendarEventBatch, CalendarEventSource, CalendarEventSourceError},
            use_cases::CalendarEventUseCases,
        },
        daily_bar_source::DateRange,
        macro_reference::MacroReferenceUseCases,
        unit_of_work::{FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction},
    };
    use core_domain::calendar_event::{
        CalendarEvent, CalendarEventReadModel, CalendarEventReference,
    };
    use tokio::sync::Mutex;

    use super::{CalendarEventIngestStats, ingest_calendar_events_with_macro_references};

    struct RecordingCalendarSource(Arc<Mutex<Vec<String>>>);

    #[async_trait]
    impl CalendarEventSource for RecordingCalendarSource {
        fn source(&self) -> &str {
            "boj"
        }

        async fn fetch_calendar_events(
            &self,
            today: NaiveDate,
        ) -> Result<CalendarEventBatch, CalendarEventSourceError> {
            self.0.lock().await.push("calendar".to_owned());
            Ok(CalendarEventBatch {
                date_range: DateRange {
                    from: today,
                    to: today,
                },
                events: Vec::new(),
            })
        }
    }

    struct RecordingCalendarRepository(Arc<Mutex<Vec<String>>>);

    #[async_trait]
    impl CalendarEventRepository for RecordingCalendarRepository {
        async fn list_events(
            &self,
            _date_range: &DateRange,
        ) -> Result<Vec<CalendarEvent>, CalendarEventRepositoryError> {
            Ok(Vec::new())
        }

        async fn enrich_events(
            &self,
            events: Vec<CalendarEvent>,
        ) -> Result<Vec<CalendarEventReadModel>, CalendarEventRepositoryError> {
            Ok(events.into_iter().map(Into::into).collect())
        }

        async fn list_references_for_event(
            &self,
            _event_source: &str,
            _event_external_id: &str,
        ) -> Result<Vec<CalendarEventReference>, CalendarEventRepositoryError> {
            Ok(Vec::new())
        }

        async fn find_next_earnings_for_stock(
            &self,
            _stock_id: &str,
            _from: NaiveDate,
        ) -> Result<Option<CalendarEvent>, CalendarEventRepositoryError> {
            Ok(None)
        }

        async fn upsert(
            &self,
            _transaction: &UnitOfWorkTransaction,
            events: Vec<CalendarEvent>,
        ) -> Result<usize, CalendarEventRepositoryError> {
            self.0.lock().await.push("upsert".to_owned());
            Ok(events.len())
        }

        async fn upsert_references(
            &self,
            _transaction: &UnitOfWorkTransaction,
            references: Vec<CalendarEventReference>,
        ) -> Result<usize, CalendarEventRepositoryError> {
            Ok(references.len())
        }

        async fn replace_macro_references_for_events(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _event_keys: Vec<(String, String)>,
            references: Vec<CalendarEventReference>,
        ) -> Result<usize, CalendarEventRepositoryError> {
            Ok(references.len())
        }

        async fn delete_missing_future_events(
            &self,
            _transaction: &UnitOfWorkTransaction,
            _source: &str,
            _date_range: &DateRange,
            _today: NaiveDate,
            _external_ids: Vec<String>,
        ) -> Result<u64, CalendarEventRepositoryError> {
            self.0.lock().await.push("delete_future".to_owned());
            Ok(0)
        }
    }

    struct RecordingTargetRepository(Arc<Mutex<Vec<String>>>);

    #[async_trait]
    impl MacroReferenceTargetRepository for RecordingTargetRepository {
        async fn list_pending_macro_reference_targets(
            &self,
            _today: NaiveDate,
            selector: &MacroReferenceTargetSelector,
        ) -> Result<Vec<MacroReferenceTarget>, CalendarEventRepositoryError> {
            self.0
                .lock()
                .await
                .push(format!("references:{}", selector.event_source));
            Ok(Vec::new())
        }

        async fn mark_macro_reference_targets_checked(
            &self,
            _targets: &[MacroReferenceTarget],
            _checked_at: DateTime<chrono::Utc>,
        ) -> Result<(), CalendarEventRepositoryError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn checks_official_references_after_the_calendar_ingest() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let repository: SharedCalendarEventRepository =
            Arc::new(RecordingCalendarRepository(calls.clone()));
        let unit_of_work: SharedUnitOfWork = Arc::new(FakeUnitOfWork::new());
        let calendar_events = CalendarEventUseCases::new(repository.clone(), unit_of_work.clone());
        let references = CalendarEventReferenceUseCases::new(repository, unit_of_work);
        let targets: Arc<dyn MacroReferenceTargetRepository> =
            Arc::new(RecordingTargetRepository(calls.clone()));
        let macro_references = MacroReferenceUseCases::new(targets, references, Vec::new());
        let calendar_source = RecordingCalendarSource(calls.clone());

        let actual = ingest_calendar_events_with_macro_references(
            &calendar_events,
            &calendar_source,
            Some(&macro_references),
        )
        .await;

        assert_eq!(
            (actual, calls.lock().await.clone()),
            (
                Ok(CalendarEventIngestStats {
                    upserted: 0,
                    deleted: 0,
                }),
                vec![
                    "calendar".to_owned(),
                    "upsert".to_owned(),
                    "delete_future".to_owned(),
                    "references:boj".to_owned(),
                ],
            ),
        );
    }
}
