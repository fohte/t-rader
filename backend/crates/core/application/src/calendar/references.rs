use chrono::Utc;
use core_domain::calendar_event::CalendarEventReference;

use crate::{
    calendar::{
        error::CalendarEventReferenceUseCaseError, repository::SharedCalendarEventRepository,
    },
    unit_of_work::SharedUnitOfWork,
};

#[derive(Clone)]
pub struct CalendarEventReferenceUseCases {
    repository: SharedCalendarEventRepository,
    unit_of_work: SharedUnitOfWork,
}

impl CalendarEventReferenceUseCases {
    pub fn new(repository: SharedCalendarEventRepository, unit_of_work: SharedUnitOfWork) -> Self {
        Self {
            repository,
            unit_of_work,
        }
    }

    pub async fn list_for_event(
        &self,
        event_source: &str,
        event_external_id: &str,
    ) -> Result<Vec<CalendarEventReference>, CalendarEventReferenceUseCaseError> {
        self.repository
            .list_references_for_event(event_source, event_external_id)
            .await
            .map_err(Into::into)
    }

    pub async fn upsert(
        &self,
        references: Vec<CalendarEventReference>,
    ) -> Result<usize, CalendarEventReferenceUseCaseError> {
        if references.is_empty() {
            return Ok(0);
        }
        if references
            .iter()
            .any(|reference| !is_valid_reference(reference))
        {
            return Err(CalendarEventReferenceUseCaseError::InvalidReference);
        }

        let transaction = self.unit_of_work.begin().await?;
        let count = self
            .repository
            .upsert_references(&transaction, references)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(count)
    }
}

fn is_valid_reference(reference: &CalendarEventReference) -> bool {
    !reference.event_source.trim().is_empty()
        && !reference.event_external_id.trim().is_empty()
        && !reference.source.trim().is_empty()
        && !reference.external_id.trim().is_empty()
        && !reference.title.trim().is_empty()
        && reference.checked_at <= Utc::now()
        && (reference.kind
            != core_domain::calendar_event::CalendarEventReferenceKind::IndividualDocument
            || reference.publication_evidence.is_some())
        && reference.url.as_deref().is_none_or(is_http_url)
        && reference
            .publication_evidence_url
            .as_deref()
            .is_none_or(is_http_url)
        && (reference.target_period_start.is_none()
            || reference.target_period_end.is_none()
            || reference.target_period_start <= reference.target_period_end)
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::NaiveDate;
    use core_domain::calendar_event::{
        CalendarEvent, CalendarEventReadModel, CalendarEventReference, CalendarEventReferenceKind,
    };

    use crate::{
        calendar::repository::{
            CalendarEventRepository, CalendarEventRepositoryError, SharedCalendarEventRepository,
        },
        daily_bar_source::DateRange,
        unit_of_work::{FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction},
    };

    use super::CalendarEventReferenceUseCases;

    struct FakeCalendarEventRepository;

    #[async_trait]
    impl CalendarEventRepository for FakeCalendarEventRepository {
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

    #[tokio::test]
    async fn individual_document_requires_evidence_before_transaction() {
        let repository = Arc::new(FakeCalendarEventRepository);
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let use_cases = CalendarEventReferenceUseCases::new(
            repository as SharedCalendarEventRepository,
            unit_of_work.clone() as SharedUnitOfWork,
        );
        let reference = CalendarEventReference {
            event_source: "sample_source".into(),
            event_external_id: "sample-event".into(),
            kind: CalendarEventReferenceKind::IndividualDocument,
            source: "sample_source".into(),
            external_id: "sample-document".into(),
            title: "架空の資料".into(),
            url: Some("https://example.invalid/document".into()),
            target_period_start: None,
            target_period_end: None,
            target_period_type: None,
            publication_evidence: None,
            publication_evidence_url: None,
            checked_at: chrono::DateTime::from_timestamp(1_700_000_000, 0)
                .expect("valid timestamp"),
        };

        let actual = (
            use_cases
                .upsert(vec![reference])
                .await
                .err()
                .map(|error| error.to_string()),
            unit_of_work.begun.lock().await.clone(),
        );

        assert_eq!(
            actual,
            (
                Some("calendar event reference is invalid".to_owned()),
                Vec::new(),
            ),
        );
    }
}
