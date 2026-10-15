use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub event_id: Uuid,
    pub event_type: String,
    pub payload: Value,
    pub chain_depth: i64,
    pub occurred_at: DateTime<FixedOffset>,
    pub dispatched_at: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewEvent {
    pub event_type: String,
    pub payload: Value,
    pub chain_depth: Option<i64>,
}

#[derive(Debug, Error)]
pub enum EventRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait EventRepository: Send + Sync {
    async fn create(
        &self,
        transaction: &UnitOfWorkTransaction,
        event: NewEvent,
    ) -> Result<Event, EventRepositoryError>;

    async fn lock_next_undispatched(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<Option<Event>, EventRepositoryError>;

    async fn mark_dispatched(
        &self,
        transaction: &UnitOfWorkTransaction,
        event_id: Uuid,
        dispatched_at: DateTime<FixedOffset>,
    ) -> Result<bool, EventRepositoryError>;

    async fn delete_before(
        &self,
        occurred_before: DateTime<FixedOffset>,
    ) -> Result<u64, EventRepositoryError>;

    async fn insert_firing(
        &self,
        transaction: &UnitOfWorkTransaction,
        event_id: Uuid,
        trigger_id: Uuid,
        task_id: Uuid,
    ) -> Result<bool, EventRepositoryError>;

    async fn has_firing(
        &self,
        transaction: &UnitOfWorkTransaction,
        event_id: Uuid,
        trigger_id: Uuid,
    ) -> Result<bool, EventRepositoryError>;
}

pub type SharedEventRepository = std::sync::Arc<dyn EventRepository + Send + Sync>;

pub(crate) fn is_valid_event_type(event_type: &str) -> bool {
    fn valid_segment(segment: &str) -> bool {
        let mut bytes = segment.bytes();
        matches!(bytes.next(), Some(b'a'..=b'z'))
            && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    }

    let mut segments = event_type.split('.');
    segments.next().is_some_and(valid_segment)
        && segments.next().is_some_and(valid_segment)
        && segments.all(valid_segment)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::is_valid_event_type;

    #[rstest]
    #[case::namespaced("sample_process.finished", true)]
    #[case::multiple_namespaces("sample_process.stage.finished", true)]
    #[case::missing_namespace("finished", false)]
    #[case::empty_segment("sample_process..finished", false)]
    #[case::uppercase("Sample_process.finished", false)]
    #[case::leading_digit("2sample.finished", false)]
    fn event_type_matches_the_text_format(#[case] event_type: &str, #[case] expected: bool) {
        assert_eq!(is_valid_event_type(event_type), expected);
    }
}
