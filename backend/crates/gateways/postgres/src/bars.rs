use std::collections::HashSet;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDate};
use core_application::bars::{
    BarsByInstrumentsQuery, BarsQuery, BarsRepository, BarsRepositoryError,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use core_domain::bar::{Bar, Timeframe};
use core_domain::instrument::Market;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, FromQueryResult, QueryFilter, Set,
    Statement,
};

use crate::DatabaseHandle;
use crate::entities::{instruments, jquants_daily_bars_ingested_date};
use crate::persistence::persistence_error;
use crate::repositories::bars::{self as bar_queries, BarsQuery as PostgresBarsQuery};
use crate::transaction::transaction_ref as postgres_transaction_ref;

#[derive(Clone)]
pub struct PostgresBarsRepository {
    db: DatabaseHandle,
}

impl PostgresBarsRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl BarsRepository for PostgresBarsRepository {
    async fn find_bars(&self, query: BarsQuery) -> Result<Vec<Bar>, BarsRepositoryError> {
        let Ok(timeframe) = query.timeframe.parse::<Timeframe>() else {
            return Ok(Vec::new());
        };

        if timeframe != Timeframe::Daily {
            let instrument = instruments::Entity::find_by_id(&query.instrument_id)
                .one(&self.db)
                .await
                .map_err(repository_error)?;
            if instrument.is_none_or(|instrument| instrument.market != Market::Us.to_string()) {
                return Ok(Vec::new());
            }

            return bar_queries::find_intraday_bars(&self.db, to_postgres_query(query))
                .await
                .map_err(repository_error);
        }

        bar_queries::find_bars(&self.db, to_postgres_query(query))
            .await
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(repository_error)
    }

    async fn find_bars_by_instruments(
        &self,
        query: BarsByInstrumentsQuery,
    ) -> Result<Vec<Bar>, BarsRepositoryError> {
        bar_queries::find_bars_by_instruments(
            &self.db,
            &query.instrument_ids,
            &query.timeframe,
            query.from,
            query.to,
        )
        .await
        .map(|rows| rows.into_iter().map(Into::into).collect())
        .map_err(repository_error)
    }

    async fn find_latest_bar(
        &self,
        instrument_id: &str,
        timeframe: &str,
    ) -> Result<Option<Bar>, BarsRepositoryError> {
        bar_queries::find_latest_bar(&self.db, instrument_id, timeframe)
            .await
            .map(|row| row.map(Into::into))
            .map_err(repository_error)
    }

    async fn find_us_stock_bar_targets(
        &self,
    ) -> Result<Vec<core_application::bars::UsStockBarTarget>, BarsRepositoryError> {
        #[derive(FromQueryResult)]
        struct TargetRow {
            instrument_id: String,
            latest_daily_bar: Option<DateTime<FixedOffset>>,
            latest_minute_bar: Option<DateTime<FixedOffset>>,
        }

        let rows = self
            .db
            .query_all_raw(Statement::from_string(
                DatabaseBackend::Postgres,
                r#"SELECT
                    instruments.id AS instrument_id,
                    daily_bar.timestamp AS latest_daily_bar,
                    minute_bar.timestamp AS latest_minute_bar
                FROM instruments
                LEFT JOIN LATERAL (
                    SELECT timestamp
                    FROM bars
                    WHERE instrument_id = instruments.id AND timeframe = '1d'
                    ORDER BY timestamp DESC
                    LIMIT 1
                ) AS daily_bar ON TRUE
                LEFT JOIN LATERAL (
                    SELECT timestamp
                    FROM minute_bars
                    WHERE instrument_id = instruments.id
                    ORDER BY timestamp DESC
                    LIMIT 1
                ) AS minute_bar ON TRUE
                WHERE instruments.market = 'US'
                  AND (
                    EXISTS (
                        SELECT 1 FROM stock_group_member
                        WHERE stock_group_member.stock_id = instruments.id
                    )
                    OR EXISTS (
                        SELECT 1 FROM note_ref
                        WHERE note_ref.ref_kind = 'stock'
                          AND note_ref.ref_id = instruments.id
                    )
                  )
                ORDER BY instruments.id"#,
            ))
            .await
            .map_err(repository_error)?;

        rows.iter()
            .map(|row| {
                let row = TargetRow::from_query_result(row, "").map_err(repository_error)?;
                Ok(core_application::bars::UsStockBarTarget {
                    instrument_id: row.instrument_id,
                    latest_daily_bar: row.latest_daily_bar.map(|timestamp| timestamp.to_utc()),
                    latest_minute_bar: row.latest_minute_bar.map(|timestamp| timestamp.to_utc()),
                })
            })
            .collect()
    }

    async fn find_ingested_dates(
        &self,
        from: NaiveDate,
    ) -> Result<HashSet<NaiveDate>, BarsRepositoryError> {
        jquants_daily_bars_ingested_date::Entity::find()
            .filter(jquants_daily_bars_ingested_date::Column::Date.gte(from))
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(|row| row.date).collect())
            .map_err(repository_error)
    }

    async fn ensure_instruments_exist(
        &self,
        transaction: &UnitOfWorkTransaction,
        instrument_ids: &HashSet<String>,
    ) -> Result<(), BarsRepositoryError> {
        if instrument_ids.is_empty() {
            return Ok(());
        }

        let transaction = transaction_ref(transaction)?;
        let models = instrument_ids
            .iter()
            .map(|instrument_id| instruments::ActiveModel {
                id: Set(instrument_id.clone()),
                name: Set(instrument_id.clone()),
                market: Set("TSE".to_string()),
                sector: Set(None),
            });
        instruments::Entity::insert_many(models)
            .on_conflict(
                OnConflict::column(instruments::Column::Id)
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(transaction)
            .await
            .map(|_| ())
            .map_err(repository_error)
    }

    async fn upsert_bars(
        &self,
        transaction: &UnitOfWorkTransaction,
        bars: Vec<Bar>,
    ) -> Result<(), BarsRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        bar_queries::upsert_bars(transaction, bars)
            .await
            .map_err(repository_error)
    }

    async fn upsert_minute_bars(
        &self,
        transaction: &UnitOfWorkTransaction,
        bars: Vec<Bar>,
    ) -> Result<(), BarsRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        bar_queries::upsert_minute_bars(transaction, bars)
            .await
            .map_err(repository_error)
    }

    async fn mark_ingested(
        &self,
        transaction: &UnitOfWorkTransaction,
        date: NaiveDate,
    ) -> Result<(), BarsRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        jquants_daily_bars_ingested_date::Entity::insert(
            jquants_daily_bars_ingested_date::ActiveModel { date: Set(date) },
        )
        .on_conflict(
            OnConflict::column(jquants_daily_bars_ingested_date::Column::Date)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(transaction)
        .await
        .map(|_| ())
        .map_err(repository_error)
    }
}

fn to_postgres_query(query: BarsQuery) -> PostgresBarsQuery {
    PostgresBarsQuery {
        instrument_id: query.instrument_id,
        timeframe: query.timeframe,
        from: query.from,
        to: query.to,
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, BarsRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(BarsRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> BarsRepositoryError {
    BarsRepositoryError::Database(persistence_error(error))
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Utc};
    use core_application::bars::{BarsQuery, BarsRepository};
    use core_application::unit_of_work::UnitOfWork;
    use core_domain::bar::{Bar, Timeframe};
    use rust_decimal::Decimal;
    use std::collections::HashSet;

    use crate::{DatabaseHandle, PostgresUnitOfWork};

    use super::PostgresBarsRepository;

    #[backend_test_macros::database_test]
    async fn writes_bars_and_ingest_dates_through_the_same_transaction(db: DatabaseHandle) {
        let instrument_id = "SAMPLE-DB-ALPHA";
        let date = NaiveDate::from_ymd_opt(2099, 7, 3).expect("fixture date");
        let bar = Bar {
            instrument_id: instrument_id.to_string(),
            timeframe: Timeframe::Daily,
            timestamp: Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).expect("midnight")),
            open: Decimal::new(140, 0),
            high: Decimal::new(150, 0),
            low: Decimal::new(130, 0),
            close: Decimal::new(145, 0),
            volume: 1000,
        };
        let repository = PostgresBarsRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("begin transaction");

        repository
            .ensure_instruments_exist(&transaction, &HashSet::from([instrument_id.into()]))
            .await
            .expect("ensure instrument");
        repository
            .upsert_bars(&transaction, vec![bar.clone()])
            .await
            .expect("upsert bar");
        repository
            .mark_ingested(&transaction, date)
            .await
            .expect("mark date");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");

        let bars = repository
            .find_bars(BarsQuery {
                instrument_id: instrument_id.to_string(),
                timeframe: "1d".into(),
                from: None,
                to: None,
            })
            .await
            .expect("find bars");
        let ingested_dates = repository
            .find_ingested_dates(date)
            .await
            .expect("find ingested dates");

        assert_eq!((bars, ingested_dates), (vec![bar], HashSet::from([date])),);
    }
}
