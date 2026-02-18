use chrono::{DateTime, FixedOffset};
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};

use crate::entities::bars;
use crate::error::AppError;
use crate::models::Bar;

/// バーデータを一括 upsert する
///
/// 複合 PK (instrument_id, timeframe, timestamp) で重複排除し、
/// 既存行は OHLCV カラムを更新する。
pub async fn upsert_bars(db: &DatabaseConnection, bars_data: Vec<Bar>) -> Result<(), AppError> {
    if bars_data.is_empty() {
        return Ok(());
    }

    let active_models: Vec<bars::ActiveModel> = bars_data.into_iter().map(Into::into).collect();

    bars::Entity::insert_many(active_models)
        .on_conflict(
            OnConflict::columns([
                bars::Column::InstrumentId,
                bars::Column::Timeframe,
                bars::Column::Timestamp,
            ])
            .update_columns([
                bars::Column::Open,
                bars::Column::High,
                bars::Column::Low,
                bars::Column::Close,
                bars::Column::Volume,
            ])
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;

    Ok(())
}

/// バーデータの検索条件
pub struct BarsQuery {
    pub instrument_id: String,
    pub timeframe: String,
    pub from: Option<DateTime<FixedOffset>>,
    pub to: Option<DateTime<FixedOffset>>,
}

/// 条件に一致するバーデータを取得する
pub async fn find_bars(
    db: &DatabaseConnection,
    query: BarsQuery,
) -> Result<Vec<bars::Model>, AppError> {
    let mut select = bars::Entity::find()
        .filter(bars::Column::InstrumentId.eq(&query.instrument_id))
        .filter(bars::Column::Timeframe.eq(&query.timeframe));

    if let Some(from) = query.from {
        select = select.filter(bars::Column::Timestamp.gte(from));
    }

    if let Some(to) = query.to {
        select = select.filter(bars::Column::Timestamp.lte(to));
    }

    let results = select.order_by_asc(bars::Column::Timestamp).all(db).await?;

    Ok(results)
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Utc};
    use rust_decimal::Decimal;
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{EntityTrait, Set};
    use sqlx::PgPool;

    use super::*;
    use crate::entities::instruments;
    use crate::models::bar::Timeframe;
    use crate::testing::create_test_db;

    /// テスト用の instrument を DB に挿入する
    async fn insert_test_instrument(db: &DatabaseConnection, id: &str) {
        instruments::Entity::insert(instruments::ActiveModel {
            id: Set(id.to_string()),
            name: Set(format!("Test {id}")),
            market: Set("TSE".to_string()),
            sector: Set(None),
        })
        .on_conflict(
            OnConflict::column(instruments::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(db)
        .await
        .expect("failed to insert test instrument");
    }

    /// テスト用のバーデータを生成する
    fn make_test_bar(instrument_id: &str, date: NaiveDate, close: i64) -> Bar {
        let timestamp = date
            .and_hms_opt(0, 0, 0)
            .map(|dt| Utc.from_utc_datetime(&dt))
            .expect("invalid date");
        Bar {
            instrument_id: instrument_id.to_string(),
            timeframe: Timeframe::Daily,
            timestamp,
            open: Decimal::new(close, 0),
            high: Decimal::new(close + 10, 0),
            low: Decimal::new(close - 10, 0),
            close: Decimal::new(close, 0),
            volume: 1000,
        }
    }

    #[sqlx::test(migrations = false)]
    async fn upsert_bars_inserts_new_records(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_test_instrument(&db, "7203").await;

        let bars = vec![
            make_test_bar(
                "7203",
                NaiveDate::from_ymd_opt(2025, 1, 6).expect("invalid date"),
                100,
            ),
            make_test_bar(
                "7203",
                NaiveDate::from_ymd_opt(2025, 1, 7).expect("invalid date"),
                105,
            ),
        ];

        upsert_bars(&db, bars).await.expect("upsert failed");

        let query = BarsQuery {
            instrument_id: "7203".to_string(),
            timeframe: "1d".to_string(),
            from: None,
            to: None,
        };
        let result = find_bars(&db, query).await.expect("find failed");
        assert_eq!(result.len(), 2);
    }

    #[sqlx::test(migrations = false)]
    async fn upsert_bars_updates_existing_records(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_test_instrument(&db, "7203").await;

        let date = NaiveDate::from_ymd_opt(2025, 1, 6).expect("invalid date");

        // 初回挿入
        upsert_bars(&db, vec![make_test_bar("7203", date, 100)])
            .await
            .expect("upsert v1 failed");

        // 同じ PK で値を更新
        upsert_bars(&db, vec![make_test_bar("7203", date, 200)])
            .await
            .expect("upsert v2 failed");

        let query = BarsQuery {
            instrument_id: "7203".to_string(),
            timeframe: "1d".to_string(),
            from: None,
            to: None,
        };
        let result = find_bars(&db, query).await.expect("find failed");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].close, Decimal::new(200, 0));
    }

    #[sqlx::test(migrations = false)]
    async fn upsert_bars_with_empty_vec_is_noop(pool: PgPool) {
        let db = create_test_db(pool).await;

        let result = upsert_bars(&db, vec![]).await;
        assert!(result.is_ok());
    }

    #[sqlx::test(migrations = false)]
    async fn find_bars_filters_by_date_range(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_test_instrument(&db, "7203").await;

        let bars = vec![
            make_test_bar(
                "7203",
                NaiveDate::from_ymd_opt(2025, 1, 6).expect("invalid date"),
                100,
            ),
            make_test_bar(
                "7203",
                NaiveDate::from_ymd_opt(2025, 1, 7).expect("invalid date"),
                105,
            ),
            make_test_bar(
                "7203",
                NaiveDate::from_ymd_opt(2025, 1, 8).expect("invalid date"),
                103,
            ),
        ];
        upsert_bars(&db, bars).await.expect("upsert failed");

        let from_dt = NaiveDate::from_ymd_opt(2025, 1, 7)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc().fixed_offset());
        let to_dt = NaiveDate::from_ymd_opt(2025, 1, 7)
            .and_then(|d| d.and_hms_opt(23, 59, 59))
            .map(|dt| dt.and_utc().fixed_offset());

        let query = BarsQuery {
            instrument_id: "7203".to_string(),
            timeframe: "1d".to_string(),
            from: from_dt,
            to: to_dt,
        };
        let result = find_bars(&db, query).await.expect("find failed");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].close, Decimal::new(105, 0));
    }
}
