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

    async fn us_instrument_ids(
        &self,
        instrument_ids: &[String],
    ) -> Result<Vec<String>, BarsRepositoryError> {
        if instrument_ids.is_empty() {
            return Ok(Vec::new());
        }

        instruments::Entity::find()
            .filter(instruments::Column::Id.is_in(instrument_ids.to_vec()))
            .filter(instruments::Column::Market.eq(Market::Us.to_string()))
            .all(&self.db)
            .await
            .map(|instruments| {
                instruments
                    .into_iter()
                    .map(|instrument| instrument.id)
                    .collect()
            })
            .map_err(repository_error)
    }
}

#[async_trait]
impl BarsRepository for PostgresBarsRepository {
    async fn find_bars(&self, query: BarsQuery) -> Result<Vec<Bar>, BarsRepositoryError> {
        let Ok(timeframe) = query.timeframe.parse::<Timeframe>() else {
            return Ok(Vec::new());
        };

        if timeframe != Timeframe::Daily {
            if self
                .us_instrument_ids(std::slice::from_ref(&query.instrument_id))
                .await?
                .is_empty()
            {
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
        let Ok(timeframe) = query.timeframe.parse::<Timeframe>() else {
            return Ok(Vec::new());
        };

        if timeframe != Timeframe::Daily {
            let us_instrument_ids = self.us_instrument_ids(&query.instrument_ids).await?;

            return bar_queries::find_intraday_bars_by_instruments(
                &self.db,
                &us_instrument_ids,
                timeframe,
                query.from,
                query.to,
            )
            .await
            .map_err(repository_error);
        }

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
    use chrono::{DateTime, NaiveDate, TimeZone, Utc};
    use core_application::bars::{BarsQuery, BarsRepository, UsStockBarTarget};
    use core_application::unit_of_work::UnitOfWork;
    use core_domain::bar::{Bar, Timeframe};
    use rust_decimal::Decimal;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::EntityTrait;
    use std::collections::HashSet;

    use crate::entities::{
        group_axis, instruments, note, note_ref, stock, stock_group, stock_group_member,
    };
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
            adjustment_factor: Decimal::ONE,
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

    #[backend_test_macros::database_test]
    async fn finds_grouped_and_note_referenced_us_stocks_with_latest_bars(db: DatabaseHandle) {
        let suffix = uuid::Uuid::new_v4().simple().to_string().to_uppercase();
        let group_stock_id = format!("US:QZ-GROUP-{suffix}");
        let note_stock_id = format!("US:QZ-NOTE-{suffix}");
        let unlinked_stock_id = format!("US:QZ-UNLINKED-{suffix}");
        let wrong_kind_stock_id = format!("US:QZ-OTHER-{suffix}");
        let non_us_group_stock_id = format!("KR:QZ-GROUP-{suffix}");

        for (id, instrument_market, stock_market) in [
            (&group_stock_id, "US", "SYNTHETIC-US-EXCHANGE"),
            (&note_stock_id, "US", "SYNTHETIC-US-EXCHANGE"),
            (&unlinked_stock_id, "US", "SYNTHETIC-US-EXCHANGE"),
            (&wrong_kind_stock_id, "US", "SYNTHETIC-US-EXCHANGE"),
            (&non_us_group_stock_id, "OTHER", "SYNTHETIC-OTHER-EXCHANGE"),
        ] {
            instruments::Entity::insert(instruments::ActiveModel {
                id: Set(id.clone()),
                name: Set(id.clone()),
                market: Set(instrument_market.to_owned()),
                sector: Set(None),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert instrument");
            stock::Entity::insert(stock::ActiveModel {
                id: Set(id.clone()),
                name: Set(id.clone()),
                market: Set(Some(stock_market.to_owned())),
                created_at: NotSet,
                updated_at: NotSet,
                product_category: Set(None),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert stock");
        }

        let axis_id = uuid::Uuid::new_v4();
        group_axis::Entity::insert(group_axis::ActiveModel {
            id: Set(axis_id),
            key: Set(format!("synthetic-axis-{suffix}")),
            name: Set("Synthetic Axis".to_owned()),
            description: Set("Synthetic axis for a database test".to_owned()),
            derive_from: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert group axis");
        let group_id = uuid::Uuid::new_v4();
        stock_group::Entity::insert(stock_group::ActiveModel {
            id: Set(group_id),
            axis_id: Set(axis_id),
            key: Set(format!("synthetic-group-{suffix}")),
            name: Set("Synthetic Group".to_owned()),
            description: Set(None),
            code: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert stock group");
        for stock_id in [&group_stock_id, &non_us_group_stock_id] {
            stock_group_member::Entity::insert(stock_group_member::ActiveModel {
                stock_id: Set(stock_id.clone()),
                group_id: Set(group_id),
                created_at: NotSet,
            })
            .exec_without_returning(&db)
            .await
            .expect("insert group membership");
        }

        let note_id = uuid::Uuid::new_v4();
        note::Entity::insert(note::ActiveModel {
            id: Set(note_id),
            kind: Set(None),
            trigger: Set(None),
            trigger_label: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
            execution_id: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert note");
        for (ref_kind, ref_id) in [
            ("stock", group_stock_id.as_str()),
            ("stock", note_stock_id.as_str()),
            ("indicator", wrong_kind_stock_id.as_str()),
        ] {
            note_ref::Entity::insert(note_ref::ActiveModel {
                note_id: Set(note_id),
                ref_kind: Set(ref_kind.to_owned()),
                ref_id: Set(ref_id.to_owned()),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert note reference");
        }

        let daily_latest = Utc
            .with_ymd_and_hms(2040, 1, 2, 0, 0, 0)
            .single()
            .expect("latest daily timestamp");
        let note_daily_latest = Utc
            .with_ymd_and_hms(2040, 1, 3, 0, 0, 0)
            .single()
            .expect("note daily timestamp");
        let minute_latest = Utc
            .with_ymd_and_hms(2040, 1, 2, 15, 4, 0)
            .single()
            .expect("latest minute timestamp");
        let repository = PostgresBarsRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db);
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        repository
            .upsert_bars(
                &transaction,
                vec![
                    test_bar(
                        &group_stock_id,
                        Timeframe::Daily,
                        Utc.with_ymd_and_hms(2040, 1, 1, 0, 0, 0)
                            .single()
                            .expect("prior daily timestamp"),
                    ),
                    test_bar(&group_stock_id, Timeframe::Daily, daily_latest),
                    test_bar(&note_stock_id, Timeframe::Daily, note_daily_latest),
                ],
            )
            .await
            .expect("insert daily bars");
        repository
            .upsert_minute_bars(
                &transaction,
                vec![
                    test_bar(
                        &group_stock_id,
                        Timeframe::Minute,
                        Utc.with_ymd_and_hms(2040, 1, 2, 15, 3, 0)
                            .single()
                            .expect("prior minute timestamp"),
                    ),
                    test_bar(&group_stock_id, Timeframe::Minute, minute_latest),
                ],
            )
            .await
            .expect("insert minute bars");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");

        let actual = repository
            .find_us_stock_bar_targets()
            .await
            .expect("find US stock bar targets");

        assert_eq!(
            actual,
            vec![
                UsStockBarTarget {
                    instrument_id: group_stock_id,
                    latest_daily_bar: Some(daily_latest),
                    latest_minute_bar: Some(minute_latest),
                },
                UsStockBarTarget {
                    instrument_id: note_stock_id,
                    latest_daily_bar: Some(note_daily_latest),
                    latest_minute_bar: None,
                },
            ],
        );
    }

    fn test_bar(instrument_id: &str, timeframe: Timeframe, timestamp: DateTime<Utc>) -> Bar {
        Bar {
            instrument_id: instrument_id.to_owned(),
            timeframe,
            timestamp,
            open: Decimal::new(101, 1),
            high: Decimal::new(111, 1),
            low: Decimal::new(91, 1),
            close: Decimal::new(105, 1),
            volume: 25,
            adjustment_factor: Decimal::ONE,
        }
    }
}
