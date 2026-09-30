use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::earnings_schedule::{
    EarningsScheduleRepository, EarningsScheduleRepositoryError,
};
use core_domain::earnings_schedule::EarningsSchedule;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::{EntityTrait, QueryOrder};

use crate::DatabaseHandle;
use crate::entities::jquants_earnings_date;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresEarningsScheduleRepository {
    db: DatabaseHandle,
}

impl PostgresEarningsScheduleRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EarningsScheduleRepository for PostgresEarningsScheduleRepository {
    async fn latest_published_date(
        &self,
    ) -> Result<Option<NaiveDate>, EarningsScheduleRepositoryError> {
        jquants_earnings_date::Entity::find()
            .order_by_desc(jquants_earnings_date::Column::PubDate)
            .one(&self.db)
            .await
            .map(|row| row.map(|model| model.pub_date))
            .map_err(repository_error)
    }

    async fn upsert(
        &self,
        schedules: Vec<EarningsSchedule>,
    ) -> Result<usize, EarningsScheduleRepositoryError> {
        if schedules.is_empty() {
            return Ok(0);
        }

        let models = schedules
            .into_iter()
            .map(to_active_model)
            .collect::<Vec<_>>();
        let count = models.len();
        jquants_earnings_date::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([
                    jquants_earnings_date::Column::Code,
                    jquants_earnings_date::Column::FqName,
                    jquants_earnings_date::Column::PubDate,
                ])
                .update_columns([
                    jquants_earnings_date::Column::SchDate,
                    jquants_earnings_date::Column::Fye,
                    jquants_earnings_date::Column::CoName,
                    jquants_earnings_date::Column::CoNameEn,
                ])
                .to_owned(),
            )
            .exec_without_returning(&self.db)
            .await
            .map(|_| count)
            .map_err(repository_error)
    }
}

fn repository_error(error: sea_orm::DbErr) -> EarningsScheduleRepositoryError {
    EarningsScheduleRepositoryError::Database(persistence_error(error))
}

fn to_active_model(schedule: EarningsSchedule) -> jquants_earnings_date::ActiveModel {
    jquants_earnings_date::ActiveModel {
        code: Set(schedule.code),
        fq_name: Set(schedule.fiscal_quarter_name),
        pub_date: Set(schedule.published_date),
        sch_date: Set(schedule.scheduled_date),
        fye: Set(schedule.fiscal_year_end),
        co_name: Set(schedule.company_name),
        co_name_en: Set(schedule.company_name_en),
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::earnings_schedule::EarningsScheduleRepository;
    use core_domain::earnings_schedule::EarningsSchedule;
    use sea_orm::EntityTrait;

    use super::PostgresEarningsScheduleRepository;
    use crate::DatabaseHandle;
    use crate::entities::jquants_earnings_date;

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
        let rows = jquants_earnings_date::Entity::find()
            .all(&db)
            .await
            .expect("list rows");

        assert_eq!(
            (count, rows),
            (
                1,
                vec![jquants_earnings_date::Model {
                    code: value.code,
                    fq_name: value.fiscal_quarter_name,
                    pub_date: value.published_date,
                    sch_date: value.scheduled_date,
                    fye: value.fiscal_year_end,
                    co_name: value.company_name,
                    co_name_en: value.company_name_en,
                }],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn upsert_updates_a_matching_composite_key(db: DatabaseHandle) {
        let repository = PostgresEarningsScheduleRepository::new(db.clone());
        let published_date = date(2042, 7, 6);
        let original = schedule(published_date, Some(date(2042, 8, 10)), "架空社");
        let updated = schedule(published_date, None, "架空社更新");
        repository
            .upsert(vec![original])
            .await
            .expect("insert original row");

        let count = repository
            .upsert(vec![updated.clone()])
            .await
            .expect("update row");
        let rows = jquants_earnings_date::Entity::find()
            .all(&db)
            .await
            .expect("list rows");

        assert_eq!(
            (count, rows),
            (
                1,
                vec![jquants_earnings_date::Model {
                    code: updated.code,
                    fq_name: updated.fiscal_quarter_name,
                    pub_date: updated.published_date,
                    sch_date: updated.scheduled_date,
                    fye: updated.fiscal_year_end,
                    co_name: updated.company_name,
                    co_name_en: updated.company_name_en,
                }],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn empty_upsert_returns_zero_without_writing(db: DatabaseHandle) {
        let repository = PostgresEarningsScheduleRepository::new(db.clone());

        let count = repository.upsert(Vec::new()).await.expect("empty upsert");
        let rows = jquants_earnings_date::Entity::find()
            .all(&db)
            .await
            .expect("list rows");

        assert_eq!((count, rows), (0, Vec::new()));
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
