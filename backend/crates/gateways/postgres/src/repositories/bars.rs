mod daily_adjustment_factors;

pub use daily_adjustment_factors::find_daily_adjustment_factors_from;

use chrono::{DateTime, FixedOffset, NaiveDate};
use core_domain::bar::{Bar, Timeframe};
use rust_decimal::Decimal;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DbErr, EntityTrait, FromQueryResult,
    QueryFilter, QueryOrder, Set, Statement,
};

use crate::entities::bars;
use crate::entities::minute_bars;

const FIND_ONE_MINUTE_BARS_SQL: &str = r#"
    SELECT instrument_id, timestamp, open, high, low, close, volume
    FROM minute_bars
    WHERE instrument_id IN ({instrument_ids})
      AND (${from}::timestamptz IS NULL OR timestamp >= ${from})
      AND (${to}::timestamptz IS NULL OR timestamp <= ${to})
    ORDER BY instrument_id ASC, timestamp ASC
"#;

const FIND_AGGREGATED_MINUTE_BARS_SQL: &str = r#"
    WITH aggregated AS (
        SELECT
            instrument_id,
            time_bucket($1::interval, timestamp) AS timestamp,
            first(open, timestamp) AS open,
            MAX(high) AS high,
            MIN(low) AS low,
            last(close, timestamp) AS close,
            SUM(volume)::bigint AS volume
        FROM minute_bars
        WHERE instrument_id IN ({instrument_ids})
          AND (${from}::timestamptz IS NULL OR timestamp >= ${from})
          -- 範囲内に開始する最後の bucket も全 OHLCV を集計する。
          AND (${to}::timestamptz IS NULL OR timestamp < ${to} + $1::interval)
        GROUP BY instrument_id, time_bucket($1::interval, timestamp)
    )
    SELECT instrument_id, timestamp, open, high, low, close, volume
    FROM aggregated
    WHERE (${from}::timestamptz IS NULL OR timestamp >= ${from})
      AND (${to}::timestamptz IS NULL OR timestamp <= ${to})
    ORDER BY instrument_id ASC, timestamp ASC
"#;

#[derive(Debug, FromQueryResult)]
struct MinuteBarRow {
    instrument_id: String,
    timestamp: DateTime<FixedOffset>,
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    volume: i64,
}

impl From<Bar> for bars::ActiveModel {
    fn from(bar: Bar) -> Self {
        bars::ActiveModel {
            instrument_id: Set(bar.instrument_id),
            timeframe: Set(bar.timeframe.to_string()),
            timestamp: Set(bar.timestamp.fixed_offset()),
            open: Set(bar.open),
            high: Set(bar.high),
            low: Set(bar.low),
            close: Set(bar.close),
            volume: Set(bar.volume),
            adjustment_factor: Set(bar.adjustment_factor),
        }
    }
}

/// DB の CHECK 制約により不正な timeframe は入らない前提で、
/// パース失敗時は Daily をフォールバックとする。
impl From<bars::Model> for Bar {
    fn from(model: bars::Model) -> Self {
        Bar {
            instrument_id: model.instrument_id,
            timeframe: model.timeframe.parse().unwrap_or(Timeframe::Daily),
            timestamp: model.timestamp.to_utc(),
            open: model.open,
            high: model.high,
            low: model.low,
            close: model.close,
            volume: model.volume,
            adjustment_factor: model.adjustment_factor,
        }
    }
}

/// バーデータを一括 upsert する
///
/// 複合 PK (instrument_id, timeframe, timestamp) で重複排除し、
/// 既存行は OHLCV カラムを更新する。
pub async fn upsert_bars(
    db: &impl sea_orm::ConnectionTrait,
    bars_data: Vec<Bar>,
) -> Result<(), DbErr> {
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
                bars::Column::AdjustmentFactor,
            ])
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;

    Ok(())
}

/// 1 分足を一括 upsert する。
pub async fn upsert_minute_bars(
    db: &impl sea_orm::ConnectionTrait,
    bars_data: Vec<Bar>,
) -> Result<(), DbErr> {
    if bars_data.is_empty() {
        return Ok(());
    }

    let active_models = bars_data.into_iter().map(|bar| minute_bars::ActiveModel {
        instrument_id: Set(bar.instrument_id),
        timestamp: Set(bar.timestamp.fixed_offset()),
        open: Set(bar.open),
        high: Set(bar.high),
        low: Set(bar.low),
        close: Set(bar.close),
        volume: Set(bar.volume),
    });
    minute_bars::Entity::insert_many(active_models)
        .on_conflict(
            OnConflict::columns([
                minute_bars::Column::InstrumentId,
                minute_bars::Column::Timestamp,
            ])
            .update_columns([
                minute_bars::Column::Open,
                minute_bars::Column::High,
                minute_bars::Column::Low,
                minute_bars::Column::Close,
                minute_bars::Column::Volume,
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
    db: &impl sea_orm::ConnectionTrait,
    query: BarsQuery,
) -> Result<Vec<bars::Model>, DbErr> {
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

/// 1 分足、または 1 分足から集計した intraday bars を取得する。
pub async fn find_intraday_bars(
    db: &impl ConnectionTrait,
    query: BarsQuery,
) -> Result<Vec<Bar>, DbErr> {
    let Ok(timeframe) = query.timeframe.parse::<Timeframe>() else {
        return Ok(Vec::new());
    };

    find_intraday_bars_by_instruments(db, &[query.instrument_id], timeframe, query.from, query.to)
        .await
}

/// 複数銘柄について 1 分足、または 1 分足から集計した intraday bars を取得する。
pub async fn find_intraday_bars_by_instruments(
    db: &impl ConnectionTrait,
    instrument_ids: &[String],
    timeframe: Timeframe,
    from: Option<DateTime<FixedOffset>>,
    to: Option<DateTime<FixedOffset>>,
) -> Result<Vec<Bar>, DbErr> {
    if instrument_ids.is_empty() {
        return Ok(Vec::new());
    }

    let interval = timeframe.bucket_interval();
    let (sql_template, first_instrument_index) = match (timeframe, interval) {
        (Timeframe::Minute, None) => (FIND_ONE_MINUTE_BARS_SQL, 1),
        (_, Some(_)) => (FIND_AGGREGATED_MINUTE_BARS_SQL, 2),
        _ => return Ok(Vec::new()),
    };
    let instrument_placeholders = (first_instrument_index
        ..first_instrument_index + instrument_ids.len())
        .map(|index| format!("${index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let from_index = first_instrument_index + instrument_ids.len();
    let to_index = from_index + 1;
    let sql = sql_template
        .replace("{instrument_ids}", &instrument_placeholders)
        .replace("{from}", &from_index.to_string())
        .replace("{to}", &to_index.to_string());
    let mut values = Vec::with_capacity(to_index);

    if let Some(interval) = interval {
        values.push(interval.into());
    }
    values.extend(instrument_ids.iter().cloned().map(Into::into));
    values.push(from.into());
    values.push(to.into());

    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            values,
        ))
        .await?;

    rows.iter()
        .map(|row| {
            let row = MinuteBarRow::from_query_result(row, "")?;
            Ok(Bar {
                instrument_id: row.instrument_id,
                timeframe,
                timestamp: row.timestamp.to_utc(),
                open: row.open,
                high: row.high,
                low: row.low,
                close: row.close,
                volume: row.volume,
                adjustment_factor: Decimal::ONE,
            })
        })
        .collect()
}

/// 複数銘柄に一致するバーデータをまとめて取得する。日付範囲は全銘柄共通の条件として扱う。
/// 結果は instrument_id 昇順 → timestamp 昇順。
pub async fn find_bars_by_instruments(
    db: &impl sea_orm::ConnectionTrait,
    instrument_ids: &[String],
    timeframe: &str,
    from: Option<DateTime<FixedOffset>>,
    to: Option<DateTime<FixedOffset>>,
) -> Result<Vec<bars::Model>, DbErr> {
    let mut select = bars::Entity::find()
        .filter(bars::Column::InstrumentId.is_in(instrument_ids.to_vec()))
        .filter(bars::Column::Timeframe.eq(timeframe));

    if let Some(from) = from {
        select = select.filter(bars::Column::Timestamp.gte(from));
    }

    if let Some(to) = to {
        select = select.filter(bars::Column::Timestamp.lte(to));
    }

    let results = select
        .order_by_asc(bars::Column::InstrumentId)
        .order_by_asc(bars::Column::Timestamp)
        .all(db)
        .await?;

    Ok(results)
}

/// 銘柄の最新の日足バーを 1 件返す。無ければ `None`。
pub async fn find_latest_bar(
    db: &impl sea_orm::ConnectionTrait,
    instrument_id: &str,
    timeframe: &str,
) -> Result<Option<bars::Model>, DbErr> {
    let result = bars::Entity::find()
        .filter(bars::Column::InstrumentId.eq(instrument_id))
        .filter(bars::Column::Timeframe.eq(timeframe))
        .order_by_desc(bars::Column::Timestamp)
        .one(db)
        .await?;
    Ok(result)
}

/// 指定日以前で最新の日足バーを 1 件返す。無ければ `None`。
/// 期限日・基準日が非営業日の場合に直近の営業日の終値へフォールバックする用途を想定する。
pub async fn find_latest_bar_on_or_before(
    db: &impl sea_orm::ConnectionTrait,
    instrument_id: &str,
    timeframe: &str,
    on_or_before: NaiveDate,
) -> Result<Option<bars::Model>, DbErr> {
    let upper = on_or_before
        .and_hms_opt(0, 0, 0)
        .map(|dt| dt.and_utc().fixed_offset());

    let mut select = bars::Entity::find()
        .filter(bars::Column::InstrumentId.eq(instrument_id))
        .filter(bars::Column::Timeframe.eq(timeframe));
    if let Some(upper) = upper {
        select = select.filter(bars::Column::Timestamp.lte(upper));
    }

    let result = select
        .order_by_desc(bars::Column::Timestamp)
        .one(db)
        .await?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::instruments;
    use chrono::{NaiveDate, TimeZone, Utc};
    use core_domain::bar::Timeframe;
    use rust_decimal::Decimal;
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{EntityTrait, Set};
    /// テスト用の instrument を DB に挿入する
    async fn insert_test_instrument(db: &impl sea_orm::ConnectionTrait, id: &str) {
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
            adjustment_factor: Decimal::ONE,
        }
    }

    fn make_test_minute_bar(
        instrument_id: &str,
        timestamp: chrono::DateTime<Utc>,
        close: i64,
    ) -> Bar {
        Bar {
            instrument_id: instrument_id.to_owned(),
            timeframe: Timeframe::Minute,
            timestamp,
            open: Decimal::new(close, 0),
            high: Decimal::new(close + 10, 0),
            low: Decimal::new(close - 10, 0),
            close: Decimal::new(close, 0),
            volume: 1000,
            adjustment_factor: Decimal::ONE,
        }
    }

    #[backend_test_macros::database_test]
    async fn upsert_bars_inserts_new_records(db: gateway_postgres::DatabaseHandle) {
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

    #[backend_test_macros::database_test]
    async fn upsert_bars_updates_existing_records(db: gateway_postgres::DatabaseHandle) {
        insert_test_instrument(&db, "7203").await;

        let date = NaiveDate::from_ymd_opt(2025, 1, 6).expect("invalid date");

        let initial = make_test_bar("7203", date, 100);
        upsert_bars(&db, vec![initial])
            .await
            .expect("upsert v1 failed");

        // 同じ PK で値を更新
        let mut adjusted = make_test_bar("7203", date, 200);
        adjusted.adjustment_factor = Decimal::new(5, 1);
        upsert_bars(&db, vec![adjusted.clone()])
            .await
            .expect("upsert v2 failed");

        let query = BarsQuery {
            instrument_id: "7203".to_string(),
            timeframe: "1d".to_string(),
            from: None,
            to: None,
        };
        let result: Vec<Bar> = find_bars(&db, query)
            .await
            .expect("find failed")
            .into_iter()
            .map(Into::into)
            .collect();
        assert_eq!(result, vec![adjusted]);
    }

    #[backend_test_macros::database_test]
    async fn upsert_bars_with_empty_vec_is_noop(db: gateway_postgres::DatabaseHandle) {
        let result = upsert_bars(&db, vec![]).await;
        assert!(result.is_ok());
    }

    #[backend_test_macros::database_test]
    async fn upsert_minute_bars_updates_existing_records(db: gateway_postgres::DatabaseHandle) {
        insert_test_instrument(&db, "US:QZ7").await;
        let timestamp = Utc
            .with_ymd_and_hms(2040, 1, 2, 15, 4, 0)
            .single()
            .expect("fixture timestamp");
        let updated_bar = make_test_minute_bar("US:QZ7", timestamp, 200);

        upsert_minute_bars(&db, vec![make_test_minute_bar("US:QZ7", timestamp, 100)])
            .await
            .expect("insert minute bar");
        upsert_minute_bars(&db, vec![updated_bar.clone()])
            .await
            .expect("update minute bar");

        let result = find_intraday_bars(
            &db,
            BarsQuery {
                instrument_id: "US:QZ7".to_owned(),
                timeframe: "1m".to_owned(),
                from: None,
                to: None,
            },
        )
        .await
        .expect("find minute bars");

        assert_eq!(result, vec![updated_bar]);
    }

    #[backend_test_macros::database_test]
    async fn find_bars_filters_by_date_range(db: gateway_postgres::DatabaseHandle) {
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

    #[backend_test_macros::database_test]
    async fn find_bars_by_instruments_filters_to_requested_instruments(
        db: gateway_postgres::DatabaseHandle,
    ) {
        insert_test_instrument(&db, "7203").await;
        insert_test_instrument(&db, "9984").await;
        insert_test_instrument(&db, "6758").await;

        let date = NaiveDate::from_ymd_opt(2025, 1, 6).expect("invalid date");
        let bars = vec![
            make_test_bar("7203", date, 100),
            make_test_bar("9984", date, 200),
            make_test_bar("6758", date, 300),
        ];
        upsert_bars(&db, bars).await.expect("upsert failed");

        let result = find_bars_by_instruments(
            &db,
            &["7203".to_string(), "9984".to_string()],
            "1d",
            None,
            None,
        )
        .await
        .expect("find failed");

        let closes: Vec<(String, Decimal)> = result
            .into_iter()
            .map(|b| (b.instrument_id, b.close))
            .collect();
        assert_eq!(
            closes,
            vec![
                ("7203".to_string(), Decimal::new(100, 0)),
                ("9984".to_string(), Decimal::new(200, 0)),
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn find_latest_bar_returns_most_recent(db: gateway_postgres::DatabaseHandle) {
        insert_test_instrument(&db, "7203").await;

        let bars = vec![
            make_test_bar(
                "7203",
                NaiveDate::from_ymd_opt(2025, 1, 6).expect("invalid date"),
                100,
            ),
            make_test_bar(
                "7203",
                NaiveDate::from_ymd_opt(2025, 1, 8).expect("invalid date"),
                103,
            ),
            make_test_bar(
                "7203",
                NaiveDate::from_ymd_opt(2025, 1, 7).expect("invalid date"),
                105,
            ),
        ];
        upsert_bars(&db, bars).await.expect("upsert failed");

        let result = find_latest_bar(&db, "7203", "1d")
            .await
            .expect("find failed");
        assert_eq!(result.map(|b| b.close), Some(Decimal::new(103, 0)));
    }

    #[backend_test_macros::database_test]
    async fn find_latest_bar_returns_none_when_no_bars(db: gateway_postgres::DatabaseHandle) {
        let result = find_latest_bar(&db, "7203", "1d")
            .await
            .expect("find failed");
        assert_eq!(result, None);
    }

    #[backend_test_macros::database_test]
    async fn find_latest_bar_on_or_before_returns_latest_bar_at_or_before_date(
        db: gateway_postgres::DatabaseHandle,
    ) {
        insert_test_instrument(&db, "7203").await;

        let bars = vec![
            make_test_bar(
                "7203",
                NaiveDate::from_ymd_opt(2025, 1, 6).expect("invalid date"),
                100,
            ),
            make_test_bar(
                "7203",
                NaiveDate::from_ymd_opt(2025, 1, 8).expect("invalid date"),
                103,
            ),
        ];
        upsert_bars(&db, bars).await.expect("upsert failed");

        // 1/7 は非営業日想定 (バー無し)。1/8 以前で最新の 1/6 が返る
        let result = find_latest_bar_on_or_before(
            &db,
            "7203",
            "1d",
            NaiveDate::from_ymd_opt(2025, 1, 7).expect("invalid date"),
        )
        .await
        .expect("find failed");
        assert_eq!(result.map(|b| b.close), Some(Decimal::new(100, 0)));
    }

    #[backend_test_macros::database_test]
    async fn find_latest_bar_on_or_before_returns_none_when_no_bar_before_date(
        db: gateway_postgres::DatabaseHandle,
    ) {
        insert_test_instrument(&db, "7203").await;

        upsert_bars(
            &db,
            vec![make_test_bar(
                "7203",
                NaiveDate::from_ymd_opt(2025, 1, 8).expect("invalid date"),
                103,
            )],
        )
        .await
        .expect("upsert failed");

        let result = find_latest_bar_on_or_before(
            &db,
            "7203",
            "1d",
            NaiveDate::from_ymd_opt(2025, 1, 6).expect("invalid date"),
        )
        .await
        .expect("find failed");
        assert_eq!(result, None);
    }
}
