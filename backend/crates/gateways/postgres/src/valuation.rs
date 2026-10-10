use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::valuation::{
    DailyBarAdjustmentFactor, ValuationRepository, ValuationRepositoryError,
};
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
const FIND_DAILY_BAR_ADJUSTMENT_FACTORS_FROM_SQL: &str = "SELECT (timestamp AT TIME ZONE 'UTC')::date AS date, adjustment_factor AS factor FROM bars WHERE instrument_id = $1 AND timeframe = '1d' AND timestamp >= ($2::date::timestamp AT TIME ZONE 'UTC') AND adjustment_factor <> 1 ORDER BY timestamp ASC";

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

#[derive(Debug, FromQueryResult)]
struct DailyBarAdjustmentFactorRow {
    date: NaiveDate,
    factor: Decimal,
}

impl From<DailyBarAdjustmentFactorRow> for DailyBarAdjustmentFactor {
    fn from(row: DailyBarAdjustmentFactorRow) -> Self {
        Self {
            date: row.date,
            factor: row.factor,
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

    async fn find_daily_bar_adjustment_factors_from(
        &self,
        symbol: &str,
        from: NaiveDate,
    ) -> Result<Vec<DailyBarAdjustmentFactor>, ValuationRepositoryError> {
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                FIND_DAILY_BAR_ADJUSTMENT_FACTORS_FROM_SQL,
                [symbol.to_owned().into(), from.into()],
            ))
            .await
            .map_err(repository_error)?;

        rows.into_iter()
            .map(|row| DailyBarAdjustmentFactorRow::from_query_result(&row, "").map(Into::into))
            .collect::<Result<Vec<_>, _>>()
            .map_err(repository_error)
    }
}

fn repository_error(error: sea_orm::DbErr) -> ValuationRepositoryError {
    ValuationRepositoryError::Database(persistence_error(error))
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveTime};
    use core_application::valuation::{DailyBarAdjustmentFactor, ValuationRepository};
    use core_domain::valuation::Valuation;
    use rust_decimal::Decimal;
    use sea_orm::ActiveValue::Set;
    use sea_orm::EntityTrait;
    use uuid::Uuid;

    use crate::DatabaseHandle;
    use crate::entities::{bars, instruments};

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
    async fn finds_daily_bar_adjustment_factors_from_the_requested_date(db: DatabaseHandle) {
        let suffix = Uuid::new_v4().simple().to_string().to_uppercase();
        let symbol = suffix[..4].to_string();
        let other_symbol = suffix[4..8].to_string();
        for instrument_id in [&symbol, &other_symbol] {
            instruments::Entity::insert(instruments::ActiveModel {
                id: Set(instrument_id.to_string()),
                name: Set(instrument_id.to_string()),
                market: Set("TSE".to_string()),
                sector: Set(None),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert instrument");
        }

        let bar = |instrument_id: &str, date: NaiveDate, timeframe: &str, factor: Decimal| {
            bars::ActiveModel {
                instrument_id: Set(instrument_id.to_string()),
                timeframe: Set(timeframe.to_string()),
                timestamp: Set(date.and_time(NaiveTime::MIN).and_utc().fixed_offset()),
                open: Set(Decimal::new(100, 0)),
                high: Set(Decimal::new(100, 0)),
                low: Set(Decimal::new(100, 0)),
                close: Set(Decimal::new(100, 0)),
                volume: Set(1),
                adjustment_factor: Set(factor),
            }
        };
        bars::Entity::insert_many(vec![
            bar(&symbol, date(2099, 1, 1), "1d", Decimal::new(5, 1)),
            bar(&symbol, date(2099, 1, 2), "1d", Decimal::ONE),
            bar(&symbol, date(2099, 1, 5), "1d", Decimal::new(5, 1)),
            bar(&symbol, date(2099, 2, 3), "1d", Decimal::new(25, 2)),
            bar(&other_symbol, date(2099, 1, 7), "1d", Decimal::new(2, 1)),
        ])
        .exec_without_returning(&db)
        .await
        .expect("insert daily bars");

        let repository = PostgresValuationRepository::new(db);
        let factors = repository
            .find_daily_bar_adjustment_factors_from(&symbol, date(2099, 1, 2))
            .await
            .expect("find adjustment factors");

        assert_eq!(
            factors,
            vec![
                DailyBarAdjustmentFactor {
                    date: date(2099, 1, 5),
                    factor: Decimal::new(5, 1),
                },
                DailyBarAdjustmentFactor {
                    date: date(2099, 2, 3),
                    factor: Decimal::new(25, 2),
                },
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn upsert_updates_existing_rows(db: crate::DatabaseHandle) {
        let repository = PostgresValuationRepository::new(db);
        let symbol = Uuid::new_v4().simple().to_string()[..4].to_uppercase();
        let code = format!("{symbol}0");
        let valuation_date = date(9999, 2, 27);
        let original = valuation(code.clone(), valuation_date, Decimal::new(120, 1));
        let corrected = valuation(code.clone(), valuation_date, Decimal::new(135, 1));

        let initial_upsert = repository
            .upsert(vec![original])
            .await
            .expect("initial upsert succeeds");
        let corrected_upsert = repository
            .upsert(vec![corrected.clone()])
            .await
            .expect("corrected upsert succeeds");
        let stored = repository
            .find_by_symbol_date_range(&symbol, valuation_date, valuation_date)
            .await
            .expect("valuation query succeeds");

        assert_eq!(
            (initial_upsert, corrected_upsert, stored),
            (1, 1, vec![corrected])
        );
    }

    #[backend_test_macros::database_test]
    async fn mark_ingested_is_idempotent(db: crate::DatabaseHandle) {
        let repository = PostgresValuationRepository::new(db);
        let ingestion_date = date(9999, 12, 31);

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

        assert_eq!(ingested, vec![ingestion_date]);
    }
}
