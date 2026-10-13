use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};

use crate::{
    daily_bar_source::DateRange, persistence::PersistenceError, unit_of_work::UnitOfWorkTransaction,
};

#[derive(Debug, thiserror::Error)]
pub enum CalendarEventRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait CalendarEventRepository: Send + Sync {
    async fn list_events(
        &self,
        date_range: &DateRange,
    ) -> Result<Vec<CalendarEvent>, CalendarEventRepositoryError>;

    async fn find_next_earnings_for_stock(
        &self,
        stock_id: &str,
        from: NaiveDate,
    ) -> Result<Option<CalendarEvent>, CalendarEventRepositoryError> {
        let events = self
            .list_events(&DateRange {
                from,
                to: NaiveDate::MAX,
            })
            .await?;
        Ok(events
            .into_iter()
            .filter(|event| {
                event.category == CalendarEventCategory::Earnings
                    && event.stock_id.as_deref() == Some(stock_id)
            })
            .min_by_key(|event| event.event_date))
    }

    async fn upsert(
        &self,
        transaction: &UnitOfWorkTransaction,
        events: Vec<CalendarEvent>,
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
