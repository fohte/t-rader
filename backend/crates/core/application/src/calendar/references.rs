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
