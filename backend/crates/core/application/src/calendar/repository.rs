use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::calendar_event::{CalendarEvent, CalendarEventReadModel, CalendarEventReference};

use crate::{
    daily_bar_source::DateRange, persistence::PersistenceError, unit_of_work::UnitOfWorkTransaction,
};

#[derive(Debug, thiserror::Error)]
pub enum CalendarEventRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
    #[error("calendar event reference belongs to an unknown event")]
    MissingEvent,
    #[error("calendar event reference is invalid")]
    InvalidReference,
}

#[async_trait]
pub trait CalendarEventRepository: Send + Sync {
    async fn list_events(
        &self,
        date_range: &DateRange,
    ) -> Result<Vec<CalendarEvent>, CalendarEventRepositoryError>;

    async fn enrich_events(
        &self,
        events: Vec<CalendarEvent>,
    ) -> Result<Vec<CalendarEventReadModel>, CalendarEventRepositoryError>;

    async fn list_references_for_event(
        &self,
        event_source: &str,
        event_external_id: &str,
    ) -> Result<Vec<CalendarEventReference>, CalendarEventRepositoryError>;

    async fn find_next_earnings_for_stock(
        &self,
        stock_id: &str,
        from: NaiveDate,
    ) -> Result<Option<CalendarEvent>, CalendarEventRepositoryError>;

    async fn upsert(
        &self,
        transaction: &UnitOfWorkTransaction,
        events: Vec<CalendarEvent>,
    ) -> Result<usize, CalendarEventRepositoryError>;

    async fn upsert_references(
        &self,
        transaction: &UnitOfWorkTransaction,
        references: Vec<CalendarEventReference>,
    ) -> Result<usize, CalendarEventRepositoryError>;

    async fn replace_macro_references_for_events(
        &self,
        transaction: &UnitOfWorkTransaction,
        event_keys: Vec<(String, String)>,
        references: Vec<CalendarEventReference>,
    ) -> Result<usize, CalendarEventRepositoryError>;

    async fn delete_missing_future_events(
        &self,
        transaction: &UnitOfWorkTransaction,
        source: &str,
        date_range: &DateRange,
        today: NaiveDate,
        external_ids: Vec<String>,
    ) -> Result<u64, CalendarEventRepositoryError>;
}

pub type SharedCalendarEventRepository = std::sync::Arc<dyn CalendarEventRepository + Send + Sync>;
