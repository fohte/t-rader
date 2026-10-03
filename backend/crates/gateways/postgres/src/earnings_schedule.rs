use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::calendar::repository::{
    CalendarEventRepository, CalendarEventRepositoryError,
};
use core_application::earnings_schedule::{
    EarningsScheduleRepository, EarningsScheduleRepositoryError,
};
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};
use core_domain::earnings_schedule::EarningsSchedule;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::{EntityTrait, QueryOrder};

use crate::entities::earnings_schedule_ingested_date;
use crate::persistence::persistence_error;
use crate::{DatabaseHandle, PostgresCalendarEventRepository};

#[derive(Clone)]
pub struct PostgresEarningsScheduleRepository {
    db: DatabaseHandle,
    calendar_events: PostgresCalendarEventRepository,
}

impl PostgresEarningsScheduleRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self {
            calendar_events: PostgresCalendarEventRepository::new(db.clone()),
            db,
        }
    }
}

#[async_trait]
impl EarningsScheduleRepository for PostgresEarningsScheduleRepository {
    async fn latest_published_date(
        &self,
    ) -> Result<Option<NaiveDate>, EarningsScheduleRepositoryError> {
        earnings_schedule_ingested_date::Entity::find()
            .order_by_desc(earnings_schedule_ingested_date::Column::Date)
            .one(&self.db)
            .await
            .map(|row| row.map(|model| model.date))
            .map_err(repository_error)
    }

    async fn upsert(
        &self,
        schedules: Vec<EarningsSchedule>,
    ) -> Result<usize, EarningsScheduleRepositoryError> {
        if schedules.is_empty() {
            return Ok(0);
        }

        let count = schedules.len();
        let dates = schedules
            .iter()
            .map(|schedule| schedule.published_date)
            .collect::<std::collections::BTreeSet<_>>();
        let events = schedules
            .into_iter()
            .filter_map(to_calendar_event)
            .collect();
        self.calendar_events
            .upsert(events)
            .await
            .map_err(calendar_repository_error)?;
        earnings_schedule_ingested_date::Entity::insert_many(
            dates
                .into_iter()
                .map(|date| earnings_schedule_ingested_date::ActiveModel { date: Set(date) }),
        )
        .on_conflict(
            OnConflict::column(earnings_schedule_ingested_date::Column::Date)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&self.db)
        .await
        .map_err(repository_error)?;

        Ok(count)
    }
}

fn repository_error(error: sea_orm::DbErr) -> EarningsScheduleRepositoryError {
    EarningsScheduleRepositoryError::Database(persistence_error(error))
}

fn to_calendar_event(schedule: EarningsSchedule) -> Option<CalendarEvent> {
    Some(CalendarEvent {
        source: "jquants".into(),
        external_id: format!("{}:{}", schedule.code, schedule.fiscal_quarter_name),
        category: CalendarEventCategory::Earnings,
        country: "JP".into(),
        title: schedule.company_name,
        stock_id: Some(schedule.code),
        fiscal_period: Some(schedule.fiscal_quarter_name),
        event_date: schedule.scheduled_date?,
        event_at: None,
        time_of_day: None,
    })
}

fn calendar_repository_error(
    error: CalendarEventRepositoryError,
) -> EarningsScheduleRepositoryError {
    match error {
        CalendarEventRepositoryError::Database(error) => {
            EarningsScheduleRepositoryError::Database(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::earnings_schedule::EarningsScheduleRepository;
    use core_domain::earnings_schedule::EarningsSchedule;
    use sea_orm::{EntityTrait, QueryOrder};

    use super::PostgresEarningsScheduleRepository;
    use crate::DatabaseHandle;
    use crate::entities::{calendar_event, earnings_schedule_ingested_date};

    fn event_fields(
        row: calendar_event::Model,
    ) -> (String, String, String, String, String, NaiveDate) {
        (
            row.source,
            row.external_id,
            row.category,
            row.country,
            row.title,
            row.event_date,
        )
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn schedule(
        published_date: NaiveDate,
        scheduled_date: Option<NaiveDate>,
        company_name: &str,
    ) -> EarningsSchedule {
        EarningsSchedule {
            code: "ZZ999".into(),
            fiscal_quarter_name: "FY-QX".into(),
            published_date,
            scheduled_date,
            fiscal_year_end: "1231".into(),
            company_name: company_name.into(),
            company_name_en: "Synthetic Company".into(),
        }
    }

    #[backend_test_macros::database_test]
    async fn upsert_inserts_schedule(db: DatabaseHandle) {
        let repository = PostgresEarningsScheduleRepository::new(db.clone());
        let published_date = date(2042, 7, 6);
        let value = schedule(published_date, Some(date(2042, 8, 10)), "架空社");
        let count = repository
            .upsert(vec![value.clone()])
            .await
            .expect("insert row");
        let rows = calendar_event::Entity::find()
            .order_by_asc(calendar_event::Column::ExternalId)
            .all(&db)
            .await
            .expect("list events")
            .into_iter()
            .map(event_fields)
            .collect::<Vec<_>>();
        let dates = earnings_schedule_ingested_date::Entity::find()
            .all(&db)
            .await
            .expect("list ingested dates")
            .into_iter()
            .map(|row| row.date)
            .collect::<Vec<_>>();

        assert_eq!(
            (count, rows, dates),
            (
                1,
                vec![(
                    "jquants".into(),
                    "ZZ999:FY-QX".into(),
                    "earnings".into(),
                    "JP".into(),
                    value.company_name,
                    date(2042, 8, 10),
                )],
                vec![published_date],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn upsert_updates_a_matching_composite_key(db: DatabaseHandle) {
        let repository = PostgresEarningsScheduleRepository::new(db.clone());
        let published_date = date(2042, 7, 6);
        let original = schedule(published_date, Some(date(2042, 8, 10)), "架空社");
        let updated = schedule(published_date, Some(date(2042, 8, 12)), "架空社更新");
        repository
            .upsert(vec![original])
            .await
            .expect("insert original row");

        let count = repository
            .upsert(vec![updated.clone()])
            .await
            .expect("update row");
        let rows = calendar_event::Entity::find()
            .order_by_asc(calendar_event::Column::ExternalId)
            .all(&db)
            .await
            .expect("list events")
            .into_iter()
            .map(event_fields)
            .collect::<Vec<_>>();

        assert_eq!(
            (count, rows),
            (
                1,
                vec![(
                    "jquants".into(),
                    "ZZ999:FY-QX".into(),
                    "earnings".into(),
                    "JP".into(),
                    updated.company_name,
                    date(2042, 8, 12),
                )],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn empty_upsert_returns_zero_without_writing(db: DatabaseHandle) {
        let repository = PostgresEarningsScheduleRepository::new(db.clone());

        let count = repository.upsert(Vec::new()).await.expect("empty upsert");
        let rows = calendar_event::Entity::find()
            .all(&db)
            .await
            .expect("list events");
        let dates = earnings_schedule_ingested_date::Entity::find()
            .all(&db)
            .await
            .expect("list ingested dates");

        assert_eq!((count, rows, dates), (0, Vec::new(), Vec::new()));
    }

    #[backend_test_macros::database_test]
    async fn undecided_schedules_are_not_stored_as_events_but_advance_the_cursor(
        db: DatabaseHandle,
    ) {
        let repository = PostgresEarningsScheduleRepository::new(db.clone());
        let published_date = date(2042, 7, 6);

        let count = repository
            .upsert(vec![schedule(published_date, None, "架空社")])
            .await
            .expect("record undecided schedule");
        let events = calendar_event::Entity::find()
            .all(&db)
            .await
            .expect("list events");
        let latest_date = repository
            .latest_published_date()
            .await
            .expect("latest date");

        assert_eq!(
            (count, events, latest_date),
            (1, Vec::new(), Some(published_date))
        );
    }

    #[backend_test_macros::database_test]
    async fn latest_published_date_returns_the_maximum_date(db: DatabaseHandle) {
        let repository = PostgresEarningsScheduleRepository::new(db);
        repository
            .upsert(vec![
                schedule(date(2042, 7, 6), None, "架空社"),
                schedule(date(2042, 7, 7), None, "架空社"),
            ])
            .await
            .expect("insert schedules");

        assert_eq!(
            repository
                .latest_published_date()
                .await
                .expect("latest date"),
            Some(date(2042, 7, 7)),
        );
    }
}
