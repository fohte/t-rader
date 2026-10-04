use chrono::NaiveDate;
use serde::Serialize;

use crate::calendar::{
    error::CalendarEventUseCaseError, repository::SharedCalendarEventRepository,
    source::CalendarEventSource,
};
use crate::unit_of_work::SharedUnitOfWork;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct CalendarEventIngestStats {
    pub upserted: usize,
    pub deleted: u64,
}

#[derive(Clone)]
pub struct CalendarEventUseCases {
    repository: SharedCalendarEventRepository,
    unit_of_work: SharedUnitOfWork,
}

impl CalendarEventUseCases {
    pub fn new(repository: SharedCalendarEventRepository, unit_of_work: SharedUnitOfWork) -> Self {
        Self {
            repository,
            unit_of_work,
        }
    }

    pub async fn run_ingest_cycle(
        &self,
        source: &dyn CalendarEventSource,
        today: NaiveDate,
    ) -> Result<CalendarEventIngestStats, CalendarEventUseCaseError> {
        let source_name = source.source();
        let batch = source.fetch_calendar_events(today).await?;
        if batch.date_range.from > batch.date_range.to {
            return Err(CalendarEventUseCaseError::InvalidDateRange);
        }

        let mut external_ids = Vec::with_capacity(batch.events.len());
        for event in &batch.events {
            if event.source != source_name {
                return Err(CalendarEventUseCaseError::SourceMismatch {
                    expected: source_name.to_string(),
                    actual: event.source.clone(),
                });
            }
            external_ids.push(event.external_id.clone());
        }

        let transaction = self.unit_of_work.begin().await?;
        let upserted = self.repository.upsert(&transaction, batch.events).await?;
        let deleted = self
            .repository
            .delete_missing_future_events(
                &transaction,
                source_name,
                &batch.date_range,
                today,
                external_ids,
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;

        Ok(CalendarEventIngestStats { upserted, deleted })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use crate::{
        calendar::{
            error::CalendarEventUseCaseError,
            fake::FakeCalendarEventSource,
            repository::{
                CalendarEventRepository, CalendarEventRepositoryError,
                SharedCalendarEventRepository,
            },
            source::CalendarEventBatch,
            use_cases::CalendarEventUseCases,
        },
        daily_bar_source::DateRange,
        unit_of_work::{SharedUnitOfWork, UnitOfWork, UnitOfWorkError, UnitOfWorkTransaction},
    };
    use async_trait::async_trait;
    use chrono::NaiveDate;
    use core_domain::calendar_event::{
        CalendarEvent, CalendarEventCategory, CalendarEventTimeOfDay,
    };

    #[derive(Default)]
    struct FakeUnitOfWork {
        begun: AtomicUsize,
        committed: AtomicUsize,
    }

    impl FakeUnitOfWork {
        fn counts(&self) -> (usize, usize) {
            (
                self.begun.load(Ordering::SeqCst),
                self.committed.load(Ordering::SeqCst),
            )
        }
    }

    #[async_trait]
    impl UnitOfWork for FakeUnitOfWork {
        async fn begin(&self) -> Result<UnitOfWorkTransaction, UnitOfWorkError> {
            self.begun.fetch_add(1, Ordering::SeqCst);
            Ok(UnitOfWorkTransaction::new(()))
        }

        async fn commit(&self, transaction: UnitOfWorkTransaction) -> Result<(), UnitOfWorkError> {
            transaction
                .downcast::<()>()
                .map_err(|_| UnitOfWorkError::InvalidTransaction)?;
            self.committed.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeCalendarEventRepository {
        events: Mutex<HashMap<(String, String), CalendarEvent>>,
    }

    impl FakeCalendarEventRepository {
        fn with_events(events: Vec<CalendarEvent>) -> Self {
            Self {
                events: Mutex::new(
                    events
                        .into_iter()
                        .map(|event| ((event.source.clone(), event.external_id.clone()), event))
                        .collect(),
                ),
            }
        }

        fn snapshot(&self) -> Vec<CalendarEvent> {
            let mut events = self
                .events
                .lock()
                .expect("events lock")
                .values()
                .cloned()
                .collect::<Vec<_>>();
            events.sort_by(|left, right| {
                (&left.source, &left.external_id).cmp(&(&right.source, &right.external_id))
            });
            events
        }
    }

    #[async_trait]
    impl CalendarEventRepository for FakeCalendarEventRepository {
        async fn upsert(
            &self,
            _transaction: &UnitOfWorkTransaction,
            events: Vec<CalendarEvent>,
        ) -> Result<usize, CalendarEventRepositoryError> {
            let count = events.len();
            let mut stored = self.events.lock().expect("events lock");
            for event in events {
                stored.insert((event.source.clone(), event.external_id.clone()), event);
            }
            Ok(count)
        }

        async fn delete_missing_future_events(
            &self,
            _transaction: &UnitOfWorkTransaction,
            source: &str,
            date_range: &DateRange,
            today: NaiveDate,
            external_ids: Vec<String>,
        ) -> Result<u64, CalendarEventRepositoryError> {
            let from = date_range.from.max(today);
            if from > date_range.to {
                return Ok(0);
            }

            let retained = external_ids
                .into_iter()
                .collect::<std::collections::HashSet<_>>();
            let mut stored = self.events.lock().expect("events lock");
            let before = stored.len();
            stored.retain(|(event_source, external_id), event| {
                event_source != source
                    || event.event_date < from
                    || event.event_date > date_range.to
                    || retained.contains(external_id)
            });
            Ok(u64::try_from(before - stored.len()).expect("event count fits in u64"))
        }
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn event(source: &str, external_id: &str, title: &str, event_date: NaiveDate) -> CalendarEvent {
        CalendarEvent {
            source: source.into(),
            external_id: external_id.into(),
            category: CalendarEventCategory::Earnings,
            country: "JP".into(),
            title: title.into(),
            stock_id: Some("DEMO1".into()),
            fiscal_period: Some("QX".into()),
            event_date,
            event_at: None,
            time_of_day: Some(CalendarEventTimeOfDay::PreMarket),
        }
    }

    fn make_use_cases(
        repository: Arc<FakeCalendarEventRepository>,
    ) -> (CalendarEventUseCases, Arc<FakeUnitOfWork>) {
        let unit_of_work = Arc::new(FakeUnitOfWork::default());
        (
            CalendarEventUseCases::new(
                repository as SharedCalendarEventRepository,
                unit_of_work.clone() as SharedUnitOfWork,
            ),
            unit_of_work,
        )
    }

    #[tokio::test]
    async fn ingest_upserts_events_and_deletes_missing_future_events_in_source_range() {
        let date_range = DateRange {
            from: date(2099, 8, 1),
            to: date(2099, 8, 31),
        };
        let repository = Arc::new(FakeCalendarEventRepository::with_events(vec![
            event("jquants", "existing", "old title", date(2099, 8, 11)),
            event("jquants", "missing", "stale event", date(2099, 8, 10)),
            event("jquants", "outside", "outside range", date(2099, 9, 1)),
            event("jquants", "past", "past event", date(2099, 8, 5)),
            event("other", "other-source", "other source", date(2099, 8, 10)),
        ]));
        let source = FakeCalendarEventSource::new(
            "jquants",
            CalendarEventBatch {
                date_range,
                events: vec![
                    event("jquants", "existing", "updated title", date(2099, 8, 6)),
                    event("jquants", "new", "new event", date(2099, 8, 12)),
                ],
            },
        );
        let (use_cases, unit_of_work) = make_use_cases(repository.clone());

        let actual = (
            use_cases
                .run_ingest_cycle(&source, date(2099, 8, 10))
                .await
                .map_err(|error| error.to_string()),
            repository.snapshot(),
            unit_of_work.counts(),
        );

        assert_eq!(
            actual,
            (
                Ok(crate::calendar::use_cases::CalendarEventIngestStats {
                    upserted: 2,
                    deleted: 1,
                }),
                vec![
                    event("jquants", "existing", "updated title", date(2099, 8, 6)),
                    event("jquants", "new", "new event", date(2099, 8, 12)),
                    event("jquants", "outside", "outside range", date(2099, 9, 1)),
                    event("jquants", "past", "past event", date(2099, 8, 5)),
                    event("other", "other-source", "other source", date(2099, 8, 10)),
                ],
                (1, 1),
            ),
        );
    }

    #[tokio::test]
    async fn invalid_date_range_does_not_change_stored_events() {
        let existing = event("jquants", "existing", "existing", date(2099, 8, 5));
        let repository = Arc::new(FakeCalendarEventRepository::with_events(vec![
            existing.clone(),
        ]));
        let source = FakeCalendarEventSource::new(
            "jquants",
            CalendarEventBatch {
                date_range: DateRange {
                    from: date(2099, 8, 2),
                    to: date(2099, 8, 1),
                },
                events: vec![event("jquants", "new", "new event", date(2099, 8, 1))],
            },
        );
        let (use_cases, _) = make_use_cases(repository.clone());

        let actual = (
            use_cases
                .run_ingest_cycle(&source, date(2099, 8, 1))
                .await
                .map_err(|error| error.to_string()),
            repository.snapshot(),
        );

        assert_eq!(
            actual,
            (
                Err(CalendarEventUseCaseError::InvalidDateRange.to_string()),
                vec![existing],
            ),
        );
    }

    #[tokio::test]
    async fn source_mismatch_does_not_change_stored_events() {
        let existing = event("jquants", "existing", "existing", date(2099, 8, 5));
        let repository = Arc::new(FakeCalendarEventRepository::with_events(vec![
            existing.clone(),
        ]));
        let source = FakeCalendarEventSource::new(
            "jquants",
            CalendarEventBatch {
                date_range: DateRange {
                    from: date(2099, 8, 1),
                    to: date(2099, 8, 31),
                },
                events: vec![event("other", "new", "new event", date(2099, 8, 12))],
            },
        );
        let (use_cases, _) = make_use_cases(repository.clone());

        let actual = (
            use_cases
                .run_ingest_cycle(&source, date(2099, 8, 10))
                .await
                .map_err(|error| error.to_string()),
            repository.snapshot(),
        );

        assert_eq!(
            actual,
            (
                Err(CalendarEventUseCaseError::SourceMismatch {
                    expected: "jquants".into(),
                    actual: "other".into(),
                }
                .to_string()),
                vec![existing],
            ),
        );
    }

    #[tokio::test]
    async fn date_range_ending_before_today_keeps_events() {
        let existing = event("jquants", "existing", "existing", date(2099, 8, 5));
        let repository = Arc::new(FakeCalendarEventRepository::with_events(vec![
            existing.clone(),
        ]));
        let source = FakeCalendarEventSource::new(
            "jquants",
            CalendarEventBatch {
                date_range: DateRange {
                    from: date(2099, 8, 1),
                    to: date(2099, 8, 5),
                },
                events: Vec::new(),
            },
        );
        let (use_cases, _) = make_use_cases(repository.clone());

        let actual = (
            use_cases
                .run_ingest_cycle(&source, date(2099, 8, 10))
                .await
                .map_err(|error| error.to_string()),
            repository.snapshot(),
        );

        assert_eq!(
            actual,
            (
                Ok(crate::calendar::use_cases::CalendarEventIngestStats {
                    upserted: 0,
                    deleted: 0,
                }),
                vec![existing],
            ),
        );
    }

    #[tokio::test]
    async fn fetch_failure_preserves_stored_events() {
        let existing = event("jquants", "existing", "existing", date(2099, 8, 5));
        let repository = Arc::new(FakeCalendarEventRepository::with_events(vec![
            existing.clone(),
        ]));
        let source = FakeCalendarEventSource::failed("jquants", "source unavailable");
        let (use_cases, _) = make_use_cases(repository.clone());

        let result = use_cases.run_ingest_cycle(&source, date(2099, 8, 10)).await;
        let actual = (
            result.err().map(|error| error.to_string()),
            repository.snapshot(),
        );

        assert_eq!(
            actual,
            (
                Some(
                    CalendarEventUseCaseError::Source(
                        crate::calendar::source::CalendarEventSourceError::Failed(
                            "source unavailable".into(),
                        ),
                    )
                    .to_string(),
                ),
                vec![existing],
            ),
        );
    }
}
