use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use core_application::calendar::repository::{
    CalendarEventRepository, CalendarEventRepositoryError,
};
use core_domain::calendar_event::CalendarEvent;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::DatabaseHandle;
use crate::entities::calendar_event;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresCalendarEventRepository {
    db: DatabaseHandle,
}

impl PostgresCalendarEventRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CalendarEventRepository for PostgresCalendarEventRepository {
    async fn upsert(
        &self,
        events: Vec<CalendarEvent>,
    ) -> Result<usize, CalendarEventRepositoryError> {
        if events.is_empty() {
            return Ok(0);
        }

        let count = events.len();
        let models = events.into_iter().map(to_active_model).collect::<Vec<_>>();
        calendar_event::Entity::insert_many(models)
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
            .exec_without_returning(&self.db)
            .await
            .map(|_| count)
            .map_err(repository_error)
    }

    async fn delete_missing_future_events(
        &self,
        source: &str,
        date_range: &core_application::daily_bar_source::DateRange,
        today: NaiveDate,
        external_ids: Vec<String>,
    ) -> Result<u64, CalendarEventRepositoryError> {
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
            .exec(&self.db)
            .await
            .map(|result| result.rows_affected)
            .map_err(repository_error)
    }
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
    };
    use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};
    use sea_orm::{EntityTrait, QueryOrder};

    use super::PostgresCalendarEventRepository;
    use crate::DatabaseHandle;
    use crate::entities::calendar_event;

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
        let repository = PostgresCalendarEventRepository::new(db.clone());
        let stored = vec![
            event("jquants", "existing", "old title", date(2099, 8, 11)),
            event("jquants", "missing", "stale event", date(2099, 8, 12)),
            event("jquants", "past", "past event", date(2099, 8, 5)),
            event("jquants", "outside", "outside range", date(2099, 9, 1)),
            event("other", "other-source", "other source", date(2099, 8, 12)),
        ];
        let inserted = repository
            .upsert(stored)
            .await
            .expect("insert initial events");
        let updated = repository
            .upsert(vec![event(
                "jquants",
                "existing",
                "updated title",
                date(2099, 8, 13),
            )])
            .await
            .expect("upsert updated event");
        let deleted = repository
            .delete_missing_future_events(
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
        let repository = PostgresCalendarEventRepository::new(db.clone());
        repository
            .upsert(vec![
                event("jquants", "future", "future event", date(2099, 8, 11)),
                event("jquants", "past", "past event", date(2099, 8, 9)),
                event("jquants", "outside", "outside range", date(2099, 9, 1)),
                event("other", "other-source", "other source", date(2099, 8, 11)),
            ])
            .await
            .expect("insert initial events");

        let deleted = repository
            .delete_missing_future_events(
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
}
