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

#[derive(Debug, PartialEq, Eq, Serialize)]
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
        for failure in &stats.failures {
            tracing::warn!(
                stock_id = %failure.stock_id,
                error = %failure.error,
                "SEC filing reference fetch failed; the pending target will be retried"
            );
        }
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{DateTime, NaiveDate, Utc};
    use core_application::{
        calendar::{
            references::CalendarEventReferenceUseCases,
            repository::{
                CalendarEventRepository, CalendarEventRepositoryError,
                SharedCalendarEventRepository,
            },
            source::{CalendarEventBatch, CalendarEventSource, CalendarEventSourceError},
            use_cases::CalendarEventUseCases,
        },
        daily_bar_source::DateRange,
        sec_filing_reference::{
            SecFilingReferenceStats, SecFilingReferenceTarget, SecFilingReferenceUseCases,
            SecFilingTargetRepository,
        },
        sec_filing_source::{SecFilingDocument, SecFilingSource, SecFilingSourceError},
        unit_of_work::{FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction},
    };
    use core_domain::calendar_event::{
        CalendarEvent, CalendarEventReadModel, CalendarEventReference,
    };
    use tokio::sync::Mutex;

    use super::{AlphaVantageCalendarIngestStats, ingest_calendar_events};

    struct RecordingCalendarSource(Arc<Mutex<Vec<&'static str>>>);

    #[async_trait]
    impl CalendarEventSource for RecordingCalendarSource {
        fn source(&self) -> &str {
            "alpha_vantage"
        }

        async fn fetch_calendar_events(
            &self,
            _today: NaiveDate,
        ) -> Result<CalendarEventBatch, CalendarEventSourceError> {
            self.0.lock().await.push("calendar");
            let day = date(2099, 7, 1);
            Ok(CalendarEventBatch {
                date_range: DateRange { from: day, to: day },
                events: Vec::new(),
            })
        }
    }

    struct RecordingSecTargetRepository(Arc<Mutex<Vec<&'static str>>>);

    #[async_trait]
    impl SecFilingTargetRepository for RecordingSecTargetRepository {
        async fn list_pending_sec_filing_targets(
            &self,
            _today: NaiveDate,
        ) -> Result<Vec<SecFilingReferenceTarget>, CalendarEventRepositoryError> {
            self.0.lock().await.push("targets");
            Ok(vec![SecFilingReferenceTarget {
                event_source: "alpha_vantage".to_owned(),
                event_external_id: "US:QZ-A:2099-06-30".to_owned(),
                stock_id: "US:QZ-A".to_owned(),
                cik: "900000001".to_owned(),
                fiscal_period: "2099-06-30".to_owned(),
            }])
        }

        async fn mark_sec_filing_targets_checked(
            &self,
            _targets: &[SecFilingReferenceTarget],
            _checked_at: DateTime<Utc>,
        ) -> Result<(), CalendarEventRepositoryError> {
            Ok(())
        }
    }

    struct RecordingSecSource(Arc<Mutex<Vec<&'static str>>>);

    #[async_trait]
    impl SecFilingSource for RecordingSecSource {
        async fn fetch_periodic_reports(
            &self,
            _cik: &str,
            _ticker: &str,
            _report_dates: &[NaiveDate],
        ) -> Result<Vec<SecFilingDocument>, SecFilingSourceError> {
            self.0.lock().await.push("sec");
            Ok(Vec::new())
        }
    }

    struct EmptyCalendarRepository;

    #[async_trait]
    impl CalendarEventRepository for EmptyCalendarRepository {
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
            Ok(events.len())
        }

        async fn upsert_references(
            &self,
            _transaction: &UnitOfWorkTransaction,
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
            Ok(0)
        }
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap_or(NaiveDate::MIN)
    }

    #[tokio::test]
    async fn fetches_sec_references_after_the_calendar_ingest() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let calendar_repository: SharedCalendarEventRepository = Arc::new(EmptyCalendarRepository);
        let unit_of_work: SharedUnitOfWork = Arc::new(FakeUnitOfWork::new());
        let calendar_events =
            CalendarEventUseCases::new(calendar_repository.clone(), unit_of_work.clone());
        let references = CalendarEventReferenceUseCases::new(calendar_repository, unit_of_work);
        let targets: Arc<dyn SecFilingTargetRepository> =
            Arc::new(RecordingSecTargetRepository(calls.clone()));
        let sec_filing_references = SecFilingReferenceUseCases::new(targets, references);
        let calendar_source = RecordingCalendarSource(calls.clone());
        let sec_source = RecordingSecSource(calls.clone());

        let actual = ingest_calendar_events(
            &calendar_events,
            &calendar_source,
            &sec_filing_references,
            Some(&sec_source),
        )
        .await;

        assert_eq!(
            (actual, calls.lock().await.clone()),
            (
                Ok(AlphaVantageCalendarIngestStats {
                    calendar_events:
                        core_application::calendar::use_cases::CalendarEventIngestStats {
                            upserted: 0,
                            deleted: 0,
                        },
                    sec_filing_references: Some(SecFilingReferenceStats {
                        events_checked: 1,
                        companies_checked: 1,
                        external_failures: 0,
                        references_upserted: 0,
                        budget_exhausted: false,
                        failures: Vec::new(),
                    }),
                }),
                vec!["calendar", "targets", "sec"],
            ),
        );
    }
}
