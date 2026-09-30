use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::valuation::{ValuationRepository, ValuationRepositoryError};
use core_domain::valuation::Valuation;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, FromQueryResult, QueryFilter, Set,
    Statement, entity::prelude::Decimal,
};

use crate::DatabaseHandle;
use crate::entities::{valuation, valuation_ingested_date};
use crate::persistence::persistence_error;

const FIND_BY_SYMBOL_DATE_RANGE_SQL: &str = "SELECT code, date, eps, fwd_eps, bps, roe, fwd_roe, per, fwd_per, pbr, mkt_cap FROM valuation WHERE LEFT(code, 4) = $1 AND date >= $2 AND date <= $3 ORDER BY date DESC";

#[derive(Clone)]
pub struct PostgresValuationRepository {
    db: DatabaseHandle,
}

impl PostgresValuationRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[derive(Debug, FromQueryResult)]
struct ValuationRow {
    code: String,
    date: NaiveDate,
    eps: Option<Decimal>,
    fwd_eps: Option<Decimal>,
    bps: Option<Decimal>,
    roe: Option<Decimal>,
    fwd_roe: Option<Decimal>,
    per: Option<Decimal>,
    fwd_per: Option<Decimal>,
    pbr: Option<Decimal>,
    mkt_cap: Option<Decimal>,
}

impl From<ValuationRow> for Valuation {
    fn from(row: ValuationRow) -> Self {
        Self {
            code: row.code,
            date: row.date,
            eps: row.eps,
            fwd_eps: row.fwd_eps,
            bps: row.bps,
            roe: row.roe,
            fwd_roe: row.fwd_roe,
            per: row.per,
            fwd_per: row.fwd_per,
            pbr: row.pbr,
            mkt_cap: row.mkt_cap,
        }
    }
}

#[async_trait]
impl ValuationRepository for PostgresValuationRepository {
    async fn find_ingested_dates(
        &self,
        from: NaiveDate,
    ) -> Result<Vec<NaiveDate>, ValuationRepositoryError> {
        valuation_ingested_date::Entity::find()
            .filter(valuation_ingested_date::Column::Date.gte(from))
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(|row| row.date).collect())
            .map_err(repository_error)
    }

    async fn mark_ingested(&self, date: NaiveDate) -> Result<(), ValuationRepositoryError> {
        valuation_ingested_date::Entity::insert(valuation_ingested_date::ActiveModel {
            date: Set(date),
        })
        .on_conflict(
            OnConflict::column(valuation_ingested_date::Column::Date)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&self.db)
        .await
        .map(|_| ())
        .map_err(repository_error)
    }

    async fn upsert(&self, items: Vec<Valuation>) -> Result<usize, ValuationRepositoryError> {
        if items.is_empty() {
            return Ok(0);
        }

        let models = items
            .into_iter()
            .map(|item| valuation::ActiveModel {
                code: Set(item.code),
                date: Set(item.date),
                eps: Set(item.eps),
                fwd_eps: Set(item.fwd_eps),
                bps: Set(item.bps),
                roe: Set(item.roe),
                fwd_roe: Set(item.fwd_roe),
                per: Set(item.per),
                fwd_per: Set(item.fwd_per),
                pbr: Set(item.pbr),
                mkt_cap: Set(item.mkt_cap),
            })
            .collect::<Vec<_>>();
        let row_count = models.len();

        valuation::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([valuation::Column::Code, valuation::Column::Date])
                    .update_columns([
                        valuation::Column::Eps,
                        valuation::Column::FwdEps,
                        valuation::Column::Bps,
                        valuation::Column::Roe,
                        valuation::Column::FwdRoe,
                        valuation::Column::Per,
                        valuation::Column::FwdPer,
                        valuation::Column::Pbr,
                        valuation::Column::MktCap,
                    ])
                    .to_owned(),
            )
            .exec_without_returning(&self.db)
            .await
            .map_err(repository_error)?;

        Ok(row_count)
    }

    async fn find_by_symbol_date_range(
        &self,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<Valuation>, ValuationRepositoryError> {
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                FIND_BY_SYMBOL_DATE_RANGE_SQL,
                [symbol.to_owned().into(), from.into(), to.into()],
            ))
            .await
            .map_err(repository_error)?;

        rows.into_iter()
            .map(|row| ValuationRow::from_query_result(&row, "").map(Into::into))
            .collect::<Result<Vec<_>, _>>()
            .map_err(repository_error)
    }
}

fn repository_error(error: sea_orm::DbErr) -> ValuationRepositoryError {
    ValuationRepositoryError::Database(persistence_error(error))
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::valuation::ValuationRepository;
    use core_domain::valuation::Valuation;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    use super::PostgresValuationRepository;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn valuation(code: String, date: NaiveDate, eps: Decimal) -> Valuation {
        Valuation {
            code,
            date,
            eps: Some(eps),
            fwd_eps: None,
            bps: None,
            roe: None,
            fwd_roe: None,
            per: None,
            fwd_per: None,
            pbr: None,
            mkt_cap: None,
        }
    }

    #[backend_test_macros::database_test]
    async fn find_by_symbol_date_range_filters_and_orders_rows(db: crate::DatabaseHandle) {
        let repository = PostgresValuationRepository::new(db);
        let symbol = Uuid::new_v4().simple().to_string()[..4].to_uppercase();
        let in_range_latest =
            valuation(format!("{symbol}1"), date(2099, 1, 8), Decimal::new(185, 1));
        let in_range_earlier =
            valuation(format!("{symbol}0"), date(2099, 1, 3), Decimal::new(125, 1));
        let outside_before = valuation(format!("{symbol}0"), date(2099, 1, 1), Decimal::new(90, 1));
        let outside_after = valuation(
            format!("{symbol}0"),
            date(2099, 1, 11),
            Decimal::new(210, 1),
        );
        let other_symbol = valuation(
            format!(
                "{}0",
                Uuid::new_v4().simple().to_string()[..4].to_uppercase()
            ),
            date(2099, 1, 5),
            Decimal::new(150, 1),
        );
        let inserted = vec![
            in_range_latest.clone(),
            in_range_earlier.clone(),
            outside_before,
            outside_after,
            other_symbol,
        ];
        let upserted = repository.upsert(inserted).await.expect("upsert succeeds");

        let rows = repository
            .find_by_symbol_date_range(&symbol, date(2099, 1, 2), date(2099, 1, 10))
            .await
            .expect("query succeeds");

        assert_eq!(
            (upserted, rows),
            (5, vec![in_range_latest, in_range_earlier])
        );
    }

    #[backend_test_macros::database_test]
    async fn upsert_updates_existing_rows_and_mark_ingested_is_idempotent(
        db: crate::DatabaseHandle,
    ) {
        let repository = PostgresValuationRepository::new(db);
        let symbol = Uuid::new_v4().simple().to_string()[..4].to_uppercase();
        let code = format!("{symbol}0");
        let ingestion_date = date(9999, 2, 27);
        let original = valuation(code.clone(), ingestion_date, Decimal::new(120, 1));
        let corrected = valuation(code.clone(), ingestion_date, Decimal::new(135, 1));

        let initial_upsert = repository
            .upsert(vec![original])
            .await
            .expect("initial upsert succeeds");
        repository
            .upsert(vec![corrected.clone()])
            .await
            .expect("corrected upsert succeeds");
        repository
            .mark_ingested(ingestion_date)
            .await
            .expect("first mark succeeds");
        repository
            .mark_ingested(ingestion_date)
            .await
            .expect("duplicate mark succeeds");
        let ingested = repository
            .find_ingested_dates(ingestion_date)
            .await
            .expect("ingested dates query succeeds");
        let stored = repository
            .find_by_symbol_date_range(&symbol, ingestion_date, ingestion_date)
            .await
            .expect("valuation query succeeds");

        assert_eq!(
            (initial_upsert, ingested, stored),
            (1, vec![ingestion_date], vec![corrected])
        );
    }
}
