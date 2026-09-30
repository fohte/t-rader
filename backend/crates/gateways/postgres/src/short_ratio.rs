use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::short_ratio::{
    ShortRatioQuery, ShortRatioRepository, ShortRatioRepositoryError,
};
use core_domain::short_ratio::ShortRatio;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::DatabaseHandle;
use crate::entities::short_ratio;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresShortRatioRepository {
    db: DatabaseHandle,
}

impl PostgresShortRatioRepository {
    pub fn new(db: impl Into<DatabaseHandle>) -> Self {
        Self { db: db.into() }
    }
}

#[async_trait]
impl ShortRatioRepository for PostgresShortRatioRepository {
    async fn find_latest_date(&self) -> Result<Option<NaiveDate>, ShortRatioRepositoryError> {
        crate::repositories::short_ratio::find_latest_date(&self.db)
            .await
            .map_err(repository_error)
    }

    async fn upsert(&self, ratios: Vec<ShortRatio>) -> Result<(), ShortRatioRepositoryError> {
        crate::repositories::short_ratio::upsert_short_ratios(&self.db, ratios)
            .await
            .map_err(repository_error)
    }

    async fn read(
        &self,
        query: ShortRatioQuery,
    ) -> Result<Vec<ShortRatio>, ShortRatioRepositoryError> {
        let mut select = short_ratio::Entity::find()
            .filter(short_ratio::Column::Sector33Code.eq(query.sector33_code));
        if let Some(from) = query.from {
            select = select.filter(short_ratio::Column::Date.gte(from));
        }
        if let Some(to) = query.to {
            select = select.filter(short_ratio::Column::Date.lte(to));
        }

        select
            .order_by_desc(short_ratio::Column::Date)
            .limit(query.limit)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_domain).collect())
            .map_err(repository_error)
    }
}

fn repository_error(error: sea_orm::DbErr) -> ShortRatioRepositoryError {
    ShortRatioRepositoryError::Database(persistence_error(error))
}

fn to_domain(row: short_ratio::Model) -> ShortRatio {
    ShortRatio {
        date: row.date,
        sector33_code: row.sector33_code,
        sell_excluding_short_value: row.sell_excluding_short_value,
        short_with_restriction_value: row.short_with_restriction_value,
        short_without_restriction_value: row.short_without_restriction_value,
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::short_ratio::{ShortRatioQuery, ShortRatioRepository};
    use core_domain::short_ratio::ShortRatio;
    use rust_decimal::Decimal;

    use super::PostgresShortRatioRepository;

    fn date(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, 1, day).unwrap()
    }

    fn ratio(date: NaiveDate, sector33_code: &str, value: i64) -> ShortRatio {
        ShortRatio {
            date,
            sector33_code: sector33_code.into(),
            sell_excluding_short_value: Some(Decimal::from(value)),
            short_with_restriction_value: Some(Decimal::from(value)),
            short_without_restriction_value: Some(Decimal::from(value)),
        }
    }

    #[backend_test_macros::database_test]
    async fn reads_filtered_rows_in_descending_date_order(db: crate::DatabaseHandle) {
        let repository = PostgresShortRatioRepository::new(db);
        repository
            .upsert(vec![
                ratio(date(2), "T001", 20),
                ratio(date(3), "T001", 30),
                ratio(date(4), "T002", 40),
            ])
            .await
            .expect("seed ratios");

        let rows = repository
            .read(ShortRatioQuery {
                sector33_code: "T001".into(),
                from: Some(date(2)),
                to: Some(date(3)),
                limit: 10,
            })
            .await
            .expect("read ratios");

        assert_eq!(
            rows,
            vec![ratio(date(3), "T001", 30), ratio(date(2), "T001", 20)]
        );
    }
}
