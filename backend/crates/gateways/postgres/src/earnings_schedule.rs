use async_trait::async_trait;
use chrono::{Datelike, NaiveDate};
use core_application::calendar::repository::{
    CalendarEventRepository, CalendarEventRepositoryError,
};
use core_application::earnings_schedule::{
    EarningsScheduleRepository, EarningsScheduleRepositoryError, JQUANTS_EARNINGS_SOURCE,
};
use core_application::unit_of_work::{UnitOfWork, UnitOfWorkTransaction};
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};
use core_domain::earnings_schedule::EarningsSchedule;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder};

use crate::entities::calendar_event;
use crate::entities::earnings_schedule_ingested_date;
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;
use crate::{DatabaseHandle, PostgresCalendarEventRepository, PostgresUnitOfWork};

#[derive(Clone)]
pub struct PostgresEarningsScheduleRepository {
    db: DatabaseHandle,
    calendar_events: PostgresCalendarEventRepository,
    unit_of_work: PostgresUnitOfWork,
}

impl PostgresEarningsScheduleRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self {
            calendar_events: PostgresCalendarEventRepository::new(db.clone()),
            unit_of_work: PostgresUnitOfWork::new(db.clone()),
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
            .iter()
            .map(to_calendar_event)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();
        let transaction = self.unit_of_work.begin().await?;
        let postgres_transaction = transaction_ref(&transaction)?;
        self.delete_undecided_corrections(postgres_transaction, &schedules)
            .await?;
        self.calendar_events
            .upsert(&transaction, events)
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
        .exec_without_returning(postgres_transaction)
        .await
        .map_err(repository_error)?;
        self.unit_of_work.commit(transaction).await?;

        Ok(count)
    }
}

impl PostgresEarningsScheduleRepository {
    async fn delete_undecided_corrections(
        &self,
        transaction: &DatabaseTransaction,
        schedules: &[EarningsSchedule],
    ) -> Result<(), EarningsScheduleRepositoryError> {
        for schedule in schedules
            .iter()
            .filter(|schedule| schedule.scheduled_date.is_none())
        {
            let identity_prefix = format!("{}:{}:", schedule.code, schedule.fiscal_quarter_name);
            calendar_event::Entity::delete_many()
                .filter(calendar_event::Column::Source.eq(JQUANTS_EARNINGS_SOURCE))
                .filter(calendar_event::Column::StockId.eq(Some(schedule.code.clone())))
                .filter(calendar_event::Column::ExternalId.like(format!("{identity_prefix}%")))
                .filter(calendar_event::Column::EventDate.gte(schedule.published_date))
                .exec(transaction)
                .await
                .map_err(repository_error)?;
        }

        Ok(())
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&DatabaseTransaction, EarningsScheduleRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(EarningsScheduleRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> EarningsScheduleRepositoryError {
    EarningsScheduleRepositoryError::Database(persistence_error(error))
}

fn to_calendar_event(
    schedule: &EarningsSchedule,
) -> Result<Option<CalendarEvent>, EarningsScheduleRepositoryError> {
    let Some(scheduled_date) = schedule.scheduled_date else {
        return Ok(None);
    };
    let fiscal_period = fiscal_period_end(schedule, scheduled_date)?;
    let fiscal_period = fiscal_period.format("%Y-%m-%d").to_string();

    Ok(Some(CalendarEvent {
        source: JQUANTS_EARNINGS_SOURCE.into(),
        external_id: format!(
            "{}:{}:{}",
            schedule.code, schedule.fiscal_quarter_name, fiscal_period
        ),
        category: CalendarEventCategory::Earnings,
        country: "JP".into(),
        title: schedule.company_name.clone(),
        stock_id: Some(schedule.code.clone()),
        fiscal_period: Some(fiscal_period),
        event_date: scheduled_date,
        event_at: None,
        time_of_day: None,
    }))
}

fn fiscal_period_end(
    schedule: &EarningsSchedule,
    scheduled_date: NaiveDate,
) -> Result<NaiveDate, EarningsScheduleRepositoryError> {
    let invalid_schedule = |detail: &str| {
        EarningsScheduleRepositoryError::InvalidSchedule(format!(
            "{} ({}): {detail}",
            schedule.code, schedule.fiscal_quarter_name
        ))
    };
    let quarter = match schedule.fiscal_quarter_name.as_str() {
        "FY" => 4,
        quarter_name => quarter_name
            .strip_suffix('Q')
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|quarter| (1..=4).contains(quarter))
            .ok_or_else(|| invalid_schedule("invalid fiscal quarter"))?,
    };
    let fye = schedule.fiscal_year_end.as_bytes();
    if fye.len() != 4 || !fye.iter().all(u8::is_ascii_digit) {
        return Err(invalid_schedule("invalid fiscal year end"));
    }
    let fiscal_year_end_month = schedule.fiscal_year_end[0..2]
        .parse::<i32>()
        .ok()
        .filter(|month| (1..=12).contains(month))
        .ok_or_else(|| invalid_schedule("invalid fiscal year end month"))?;

    // FYE の日部分は使わず、決算期末を月末にそろえる。
    let quarter_end_month = (fiscal_year_end_month - 1 - (4 - quarter) * 3).rem_euclid(12) + 1;
    let year = scheduled_date.year();
    let candidate = month_end(year, quarter_end_month)
        .ok_or_else(|| invalid_schedule("fiscal period is out of range"))?;
    if candidate < scheduled_date {
        return Ok(candidate);
    }

    let previous_year = year
        .checked_sub(1)
        .ok_or_else(|| invalid_schedule("fiscal period is out of range"))?;
    month_end(previous_year, quarter_end_month)
        .ok_or_else(|| invalid_schedule("fiscal period is out of range"))
}

fn month_end(year: i32, month: i32) -> Option<NaiveDate> {
    let (next_year, next_month) = if month == 12 {
        (year.checked_add(1)?, 1)
    } else {
        (year, u32::try_from(month + 1).ok()?)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)?.checked_sub_signed(chrono::Duration::days(1))
}

fn calendar_repository_error(
    error: CalendarEventRepositoryError,
) -> EarningsScheduleRepositoryError {
    match error {
        CalendarEventRepositoryError::Database(error) => {
            EarningsScheduleRepositoryError::Database(error)
        }
        CalendarEventRepositoryError::InvalidTransaction => {
            EarningsScheduleRepositoryError::InvalidTransaction
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::earnings_schedule::{
        EarningsScheduleRepository, EarningsScheduleRepositoryError,
    };
    use core_domain::earnings_schedule::EarningsSchedule;
    use sea_orm::{EntityTrait, QueryOrder};

    use super::PostgresEarningsScheduleRepository;
    use crate::DatabaseHandle;
    use crate::entities::{calendar_event, earnings_schedule_ingested_date};

    fn event_fields(
        row: calendar_event::Model,
    ) -> (
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        NaiveDate,
    ) {
        (
            row.source,
            row.external_id,
            row.category,
            row.country,
            row.title,
            row.stock_id,
            row.fiscal_period,
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
            fiscal_quarter_name: "1Q".into(),
            published_date,
            scheduled_date,
            fiscal_year_end: "1231".into(),
            company_name: company_name.into(),
            company_name_en: "Synthetic Company".into(),
        }
    }

    #[backend_test_macros::database_test]
    async fn upsert_inserts_fourth_quarter_and_fiscal_year_schedules(db: DatabaseHandle) {
        let repository = PostgresEarningsScheduleRepository::new(db.clone());
        let published_date = date(2042, 7, 6);
        let mut fourth_quarter = schedule(published_date, Some(date(2043, 1, 5)), "架空社 4Q");
        fourth_quarter.fiscal_quarter_name = "4Q".into();
        fourth_quarter.fiscal_year_end = "1230".into();
        let mut fiscal_year = schedule(published_date, Some(date(2043, 1, 5)), "架空社 FY");
        fiscal_year.fiscal_quarter_name = "FY".into();
        fiscal_year.fiscal_year_end = "1230".into();
        let count = repository
            .upsert(vec![fourth_quarter.clone(), fiscal_year.clone()])
            .await
            .expect("insert schedules");
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
                2,
                vec![
                    (
                        "jquants".into(),
                        "ZZ999:4Q:2042-12-31".into(),
                        "earnings".into(),
                        "JP".into(),
                        fourth_quarter.company_name,
                        Some("ZZ999".into()),
                        Some("2042-12-31".into()),
                        date(2043, 1, 5),
                    ),
                    (
                        "jquants".into(),
                        "ZZ999:FY:2042-12-31".into(),
                        "earnings".into(),
                        "JP".into(),
                        fiscal_year.company_name,
                        Some("ZZ999".into()),
                        Some("2042-12-31".into()),
                        date(2043, 1, 5),
                    ),
                ],
                vec![published_date],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn upsert_updates_a_matching_external_id(db: DatabaseHandle) {
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
                    "ZZ999:1Q:2042-03-31".into(),
                    "earnings".into(),
                    "JP".into(),
                    updated.company_name,
                    Some("ZZ999".into()),
                    Some("2042-03-31".into()),
                    date(2042, 8, 12),
                )],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn upsert_keeps_the_same_quarter_for_different_fiscal_years(db: DatabaseHandle) {
        let repository = PostgresEarningsScheduleRepository::new(db.clone());
        let schedules = vec![
            schedule(date(2042, 7, 6), Some(date(2042, 10, 10)), "架空社 A"),
            schedule(date(2043, 7, 6), Some(date(2043, 10, 10)), "架空社 B"),
        ];

        let count = repository
            .upsert(schedules)
            .await
            .expect("insert schedules");
        let events = calendar_event::Entity::find()
            .order_by_asc(calendar_event::Column::ExternalId)
            .all(&db)
            .await
            .expect("list events")
            .into_iter()
            .map(event_fields)
            .collect::<Vec<_>>();
        let dates = earnings_schedule_ingested_date::Entity::find()
            .order_by_asc(earnings_schedule_ingested_date::Column::Date)
            .all(&db)
            .await
            .expect("list ingested dates")
            .into_iter()
            .map(|row| row.date)
            .collect::<Vec<_>>();

        assert_eq!(
            (count, events, dates),
            (
                2,
                vec![
                    (
                        "jquants".into(),
                        "ZZ999:1Q:2042-03-31".into(),
                        "earnings".into(),
                        "JP".into(),
                        "架空社 A".into(),
                        Some("ZZ999".into()),
                        Some("2042-03-31".into()),
                        date(2042, 10, 10),
                    ),
                    (
                        "jquants".into(),
                        "ZZ999:1Q:2043-03-31".into(),
                        "earnings".into(),
                        "JP".into(),
                        "架空社 B".into(),
                        Some("ZZ999".into()),
                        Some("2043-03-31".into()),
                        date(2043, 10, 10),
                    ),
                ],
                vec![date(2042, 7, 6), date(2043, 7, 6)],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn fiscal_period_end_is_strictly_before_the_scheduled_date(db: DatabaseHandle) {
        let repository = PostgresEarningsScheduleRepository::new(db.clone());
        let scheduled_date = date(2042, 3, 31);
        repository
            .upsert(vec![schedule(
                date(2042, 3, 1),
                Some(scheduled_date),
                "架空社 A",
            )])
            .await
            .expect("insert schedule");
        let events = calendar_event::Entity::find()
            .all(&db)
            .await
            .expect("list events")
            .into_iter()
            .map(event_fields)
            .collect::<Vec<_>>();

        assert_eq!(
            events,
            vec![(
                "jquants".into(),
                "ZZ999:1Q:2041-03-31".into(),
                "earnings".into(),
                "JP".into(),
                "架空社 A".into(),
                Some("ZZ999".into()),
                Some("2041-03-31".into()),
                scheduled_date,
            )],
        );
    }

    #[backend_test_macros::database_test]
    async fn upsert_rejects_invalid_quarter_or_fiscal_year_end(db: DatabaseHandle) {
        let repository = PostgresEarningsScheduleRepository::new(db.clone());
        let mut invalid_quarter = schedule(date(2042, 7, 6), Some(date(2042, 10, 10)), "架空社 A");
        invalid_quarter.fiscal_quarter_name = "5Q".into();
        let mut invalid_fye = schedule(date(2042, 7, 7), Some(date(2042, 10, 11)), "架空社 B");
        invalid_fye.fiscal_year_end = "1331".into();

        let invalid_quarter_result = repository
            .upsert(vec![invalid_quarter])
            .await
            .map_err(|error| match error {
                EarningsScheduleRepositoryError::InvalidSchedule(message) => message,
                EarningsScheduleRepositoryError::Database(_) => "database error".into(),
                EarningsScheduleRepositoryError::UnitOfWork(_)
                | EarningsScheduleRepositoryError::InvalidTransaction => "transaction error".into(),
            })
            .map(|_| ());
        let invalid_fye_result = repository
            .upsert(vec![invalid_fye])
            .await
            .map_err(|error| match error {
                EarningsScheduleRepositoryError::InvalidSchedule(message) => message,
                EarningsScheduleRepositoryError::Database(_) => "database error".into(),
                EarningsScheduleRepositoryError::UnitOfWork(_)
                | EarningsScheduleRepositoryError::InvalidTransaction => "transaction error".into(),
            })
            .map(|_| ());
        let results = vec![invalid_quarter_result, invalid_fye_result];
        let events = calendar_event::Entity::find()
            .all(&db)
            .await
            .expect("list events");
        let dates = earnings_schedule_ingested_date::Entity::find()
            .all(&db)
            .await
            .expect("list ingested dates");

        assert_eq!(
            (results, events, dates),
            (
                vec![
                    Err("ZZ999 (5Q): invalid fiscal quarter".into()),
                    Err("ZZ999 (1Q): invalid fiscal year end month".into()),
                ],
                Vec::new(),
                Vec::new(),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn undecided_correction_deletes_matching_events_on_or_after_publication_date(
        db: DatabaseHandle,
    ) {
        let repository = PostgresEarningsScheduleRepository::new(db.clone());
        let earlier_event = schedule(date(2041, 7, 6), Some(date(2041, 10, 14)), "架空社 A");
        let later_event = schedule(date(2042, 8, 1), Some(date(2042, 10, 1)), "架空社 A");
        let mut other_quarter = schedule(date(2042, 8, 2), Some(date(2042, 10, 3)), "架空社 A");
        other_quarter.fiscal_quarter_name = "2Q".into();
        let mut other_stock = schedule(date(2042, 8, 3), Some(date(2042, 10, 4)), "架空社 B");
        other_stock.code = "AA123".into();
        let mut earlier_fiscal_year =
            schedule(date(2041, 7, 4), Some(date(2041, 10, 14)), "架空社 A");
        earlier_fiscal_year.fiscal_quarter_name = "FY".into();
        let mut later_fiscal_year = schedule(date(2042, 7, 5), Some(date(2042, 10, 5)), "架空社 A");
        later_fiscal_year.fiscal_quarter_name = "FY".into();
        repository
            .upsert(vec![
                earlier_event,
                later_event,
                other_quarter,
                other_stock,
                earlier_fiscal_year,
                later_fiscal_year,
            ])
            .await
            .expect("insert initial events");

        let published_date = date(2042, 8, 15);
        let mut fiscal_year_correction = schedule(published_date, None, "架空社 A");
        fiscal_year_correction.fiscal_quarter_name = "FY".into();
        let count = repository
            .upsert(vec![
                schedule(published_date, None, "架空社 A"),
                fiscal_year_correction,
            ])
            .await
            .expect("record undecided correction");
        let events = calendar_event::Entity::find()
            .order_by_asc(calendar_event::Column::ExternalId)
            .all(&db)
            .await
            .expect("list events")
            .into_iter()
            .map(event_fields)
            .collect::<Vec<_>>();
        let dates = earnings_schedule_ingested_date::Entity::find()
            .order_by_asc(earnings_schedule_ingested_date::Column::Date)
            .all(&db)
            .await
            .expect("list ingested dates")
            .into_iter()
            .map(|row| row.date)
            .collect::<Vec<_>>();

        assert_eq!(
            (count, events, dates),
            (
                2,
                vec![
                    (
                        "jquants".into(),
                        "AA123:1Q:2042-03-31".into(),
                        "earnings".into(),
                        "JP".into(),
                        "架空社 B".into(),
                        Some("AA123".into()),
                        Some("2042-03-31".into()),
                        date(2042, 10, 4),
                    ),
                    (
                        "jquants".into(),
                        "ZZ999:1Q:2041-03-31".into(),
                        "earnings".into(),
                        "JP".into(),
                        "架空社 A".into(),
                        Some("ZZ999".into()),
                        Some("2041-03-31".into()),
                        date(2041, 10, 14),
                    ),
                    (
                        "jquants".into(),
                        "ZZ999:2Q:2042-06-30".into(),
                        "earnings".into(),
                        "JP".into(),
                        "架空社 A".into(),
                        Some("ZZ999".into()),
                        Some("2042-06-30".into()),
                        date(2042, 10, 3),
                    ),
                    (
                        "jquants".into(),
                        "ZZ999:FY:2040-12-31".into(),
                        "earnings".into(),
                        "JP".into(),
                        "架空社 A".into(),
                        Some("ZZ999".into()),
                        Some("2040-12-31".into()),
                        date(2041, 10, 14),
                    ),
                ],
                vec![
                    date(2041, 7, 4),
                    date(2041, 7, 6),
                    date(2042, 7, 5),
                    date(2042, 8, 1),
                    date(2042, 8, 2),
                    date(2042, 8, 3),
                    published_date,
                ],
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
