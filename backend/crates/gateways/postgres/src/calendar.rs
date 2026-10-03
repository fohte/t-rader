use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use core_application::calendar::repository::{
    CalendarEventRepository, CalendarEventRepositoryError,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use core_domain::calendar_event::CalendarEvent;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, EntityTrait, Iterable, QueryFilter};

use crate::entities::calendar_event;
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

const MAX_UPSERT_ROWS_PER_STATEMENT: usize = 1_000;

#[derive(Clone, Default)]
pub struct PostgresCalendarEventRepository;

impl PostgresCalendarEventRepository {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CalendarEventRepository for PostgresCalendarEventRepository {
    async fn upsert(
        &self,
        transaction: &UnitOfWorkTransaction,
        events: Vec<CalendarEvent>,
    ) -> Result<usize, CalendarEventRepositoryError> {
        if events.is_empty() {
            return Ok(0);
        }

        let transaction = transaction_ref(transaction)?;
        let count = events.len();
        let models = events.into_iter().map(to_active_model).collect::<Vec<_>>();
        let columns_per_row = calendar_event::Column::iter().count();
        let chunk_size = (u16::MAX as usize / columns_per_row).min(MAX_UPSERT_ROWS_PER_STATEMENT);

        for chunk in models.chunks(chunk_size) {
            calendar_event::Entity::insert_many(chunk.to_vec())
                .on_conflict(
                    OnConflict::columns([
                        calendar_event::Column::Source,
                        calendar_event::Column::ExternalId,
                    ])
                    .update_columns([
                        calendar_event::Column::Category,
                        calendar_event::Column::Country,
                        calendar_event::Column::Title,
                        calendar_event::Column::StockId,
                        calendar_event::Column::FiscalPeriod,
                        calendar_event::Column::EventDate,
                        calendar_event::Column::EventAt,
                        calendar_event::Column::TimeOfDay,
                        calendar_event::Column::UpdatedAt,
                    ])
                    .to_owned(),
                )
                .exec_without_returning(transaction)
                .await
                .map_err(repository_error)?;
        }

        Ok(count)
    }

    async fn delete_missing_future_events(
        &self,
        transaction: &UnitOfWorkTransaction,
        source: &str,
        date_range: &core_application::daily_bar_source::DateRange,
        today: NaiveDate,
        external_ids: Vec<String>,
    ) -> Result<u64, CalendarEventRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let from = date_range.from.max(today);
        if from > date_range.to {
            return Ok(0);
        }

        let mut delete = calendar_event::Entity::delete_many()
            .filter(calendar_event::Column::Source.eq(source))
            .filter(calendar_event::Column::EventDate.gte(from))
            .filter(calendar_event::Column::EventDate.lte(date_range.to));
        if !external_ids.is_empty() {
            delete = delete.filter(calendar_event::Column::ExternalId.is_not_in(external_ids));
        }

        delete
            .exec(transaction)
            .await
            .map(|result| result.rows_affected)
            .map_err(repository_error)
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, CalendarEventRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(CalendarEventRepositoryError::InvalidTransaction)
}

fn to_active_model(event: CalendarEvent) -> calendar_event::ActiveModel {
    calendar_event::ActiveModel {
        source: Set(event.source),
        external_id: Set(event.external_id),
        category: Set(event.category.as_str().to_string()),
        country: Set(event.country),
        title: Set(event.title),
        stock_id: Set(event.stock_id),
        fiscal_period: Set(event.fiscal_period),
        event_date: Set(event.event_date),
        event_at: Set(event.event_at.map(|timestamp| timestamp.fixed_offset())),
        time_of_day: Set(event.time_of_day.map(|value| value.as_str().to_string())),
        updated_at: Set(Utc::now().fixed_offset()),
        ..Default::default()
    }
}

fn repository_error(error: sea_orm::DbErr) -> CalendarEventRepositoryError {
    CalendarEventRepositoryError::Database(persistence_error(error))
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::{
        calendar::repository::CalendarEventRepository, daily_bar_source::DateRange,
        unit_of_work::UnitOfWork,
    };
    use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

    use super::PostgresCalendarEventRepository;
    use crate::entities::calendar_event;
    use crate::{DatabaseHandle, PostgresUnitOfWork};

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
            time_of_day: None,
        }
    }

    fn fields(row: calendar_event::Model) -> (String, String, String, NaiveDate) {
        (row.source, row.external_id, row.title, row.event_date)
    }

    #[backend_test_macros::database_test]
    async fn upsert_replaces_matching_events_and_deletes_only_missing_future_events_in_range(
        db: DatabaseHandle,
    ) {
        let repository = PostgresCalendarEventRepository::new();
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let stored = vec![
            event("jquants", "existing", "old title", date(2099, 8, 11)),
            event("jquants", "missing", "stale event", date(2099, 8, 12)),
            event("jquants", "past", "past event", date(2099, 8, 5)),
            event("jquants", "outside", "outside range", date(2099, 9, 1)),
            event("other", "other-source", "other source", date(2099, 8, 12)),
        ];
        let inserted = repository
            .upsert(&transaction, stored)
            .await
            .expect("insert initial events");
        let updated = repository
            .upsert(
                &transaction,
                vec![event(
                    "jquants",
                    "existing",
                    "updated title",
                    date(2099, 8, 13),
                )],
            )
            .await
            .expect("upsert updated event");
        let deleted = repository
            .delete_missing_future_events(
                &transaction,
                "jquants",
                &DateRange {
                    from: date(2099, 8, 1),
                    to: date(2099, 8, 31),
                },
                date(2099, 8, 10),
                vec!["existing".into()],
            )
            .await
            .expect("delete missing future events");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");
        let rows = calendar_event::Entity::find()
            .order_by_asc(calendar_event::Column::Source)
            .order_by_asc(calendar_event::Column::ExternalId)
            .all(&db)
            .await
            .expect("list events")
            .into_iter()
            .map(fields)
            .collect::<Vec<_>>();

        assert_eq!(
            (inserted, updated, deleted, rows),
            (
                5,
                1,
                1,
                vec![
                    (
                        "jquants".into(),
                        "existing".into(),
                        "updated title".into(),
                        date(2099, 8, 13),
                    ),
                    (
                        "jquants".into(),
                        "outside".into(),
                        "outside range".into(),
                        date(2099, 9, 1),
                    ),
                    (
                        "jquants".into(),
                        "past".into(),
                        "past event".into(),
                        date(2099, 8, 5),
                    ),
                    (
                        "other".into(),
                        "other-source".into(),
                        "other source".into(),
                        date(2099, 8, 12),
                    ),
                ],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn empty_external_ids_delete_all_missing_future_events_in_source_range(
        db: DatabaseHandle,
    ) {
        let repository = PostgresCalendarEventRepository::new();
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        repository
            .upsert(
                &transaction,
                vec![
                    event("jquants", "future", "future event", date(2099, 8, 11)),
                    event("jquants", "past", "past event", date(2099, 8, 9)),
                    event("jquants", "outside", "outside range", date(2099, 9, 1)),
                    event("other", "other-source", "other source", date(2099, 8, 11)),
                ],
            )
            .await
            .expect("insert initial events");

        let deleted = repository
            .delete_missing_future_events(
                &transaction,
                "jquants",
                &DateRange {
                    from: date(2099, 8, 1),
                    to: date(2099, 8, 31),
                },
                date(2099, 8, 10),
                Vec::new(),
            )
            .await
            .expect("delete missing future events");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");
        let rows = calendar_event::Entity::find()
            .order_by_asc(calendar_event::Column::Source)
            .order_by_asc(calendar_event::Column::ExternalId)
            .all(&db)
            .await
            .expect("list events")
            .into_iter()
            .map(fields)
            .collect::<Vec<_>>();

        assert_eq!(
            (deleted, rows),
            (
                1,
                vec![
                    (
                        "jquants".into(),
                        "outside".into(),
                        "outside range".into(),
                        date(2099, 9, 1),
                    ),
                    (
                        "jquants".into(),
                        "past".into(),
                        "past event".into(),
                        date(2099, 8, 9),
                    ),
                    (
                        "other".into(),
                        "other-source".into(),
                        "other source".into(),
                        date(2099, 8, 11),
                    ),
                ],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn upsert_chunks_large_event_batches(db: DatabaseHandle) {
        let repository = PostgresCalendarEventRepository::new();
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let events = (0..6_001)
            .map(|index| {
                event(
                    "alpha_vantage_fixture",
                    &format!("fixture-{index}"),
                    "Example Holdings",
                    date(2099, 8, 11),
                )
            })
            .collect();

        let upserted = repository
            .upsert(&transaction, events)
            .await
            .expect("upsert large batch");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");
        let rows = calendar_event::Entity::find()
            .filter(calendar_event::Column::Source.eq("alpha_vantage_fixture"))
            .all(&db)
            .await
            .expect("list inserted events");

        assert_eq!((upserted, rows.len()), (6_001, 6_001));
    }
}
