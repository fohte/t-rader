mod daily_adjustment_factors;
mod timestamp_coverage;
mod us_stock_intraday;

pub use daily_adjustment_factors::find_daily_adjustment_factors_from;
pub(crate) use timestamp_coverage::{BarTimestampCoverage, upsert_bar_timestamp_coverage};

pub(crate) const OHLCV_AGGREGATES: &str = "first(open, timestamp) AS open, MAX(high) AS high, MIN(low) AS low, last(close, timestamp) AS close, SUM(volume)::bigint AS volume";

use chrono::{DateTime, FixedOffset, NaiveDate};
use core_domain::bar::{Bar, Timeframe};
use rust_decimal::Decimal;
use sea_orm::sea_query::{Expr, Iden, OnConflict};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DbErr, EntityTrait, FromQueryResult, Iterable,
    QueryFilter, QueryOrder, Set, Statement,
};

use crate::entities::bars;

const BAR_UPDATE_COLUMNS: [bars::Column; 6] = [
    bars::Column::Open,
    bars::Column::High,
    bars::Column::Low,
    bars::Column::Close,
    bars::Column::Volume,
    bars::Column::AdjustmentFactor,
];

fn changed_bar_columns_condition() -> Expr {
    let condition = BAR_UPDATE_COLUMNS
        .iter()
        .map(|column| {
            let column_name = Iden::to_string(column);
            format!("bars.{column_name} IS DISTINCT FROM EXCLUDED.{column_name}")
        })
        .collect::<Vec<_>>()
        .join(" OR ");
    Expr::cust(condition)
}

const FIND_ONE_MINUTE_BARS_SQL: &str = r#"
    SELECT instrument_id, timestamp, open, high, low, close, volume
    FROM bars
    WHERE instrument_id IN ({instrument_ids})
      AND timeframe = '1m'
      AND (${from}::timestamptz IS NULL OR timestamp >= ${from})
      AND (${to}::timestamptz IS NULL OR timestamp <= ${to})
    ORDER BY instrument_id ASC, timestamp ASC
"#;

const FIND_AGGREGATED_MINUTE_BARS_SQL: &str = r#"
    WITH aggregated AS (
        SELECT
            instrument_id,
            time_bucket($1::interval, timestamp) AS timestamp,
            {ohlcv_aggregates}
        FROM bars
        WHERE instrument_id IN ({instrument_ids})
          AND timeframe = '1m'
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

const FIND_AGGREGATED_DAILY_BARS_SQL: &str = r#"
    WITH aggregated AS (
        SELECT
            instrument_id,
            time_bucket($1::interval, timestamp) AS timestamp,
            {ohlcv_aggregates}
        FROM bars
        WHERE instrument_id IN ({instrument_ids})
          AND timeframe = '1d'
          AND (${from}::timestamptz IS NULL OR timestamp >= time_bucket($1::interval, ${from}::timestamptz))
          -- to が属する週の全 OHLCV を集計する。
          AND (${to}::timestamptz IS NULL OR timestamp < ${to} + $1::interval)
        GROUP BY instrument_id, time_bucket($1::interval, timestamp)
    )
    SELECT instrument_id, timestamp, open, high, low, close, volume
    FROM aggregated
    WHERE (${from}::timestamptz IS NULL OR timestamp >= time_bucket($1::interval, ${from}::timestamptz))
      AND (${to}::timestamptz IS NULL OR timestamp <= ${to})
    ORDER BY instrument_id ASC, timestamp ASC
"#;

#[derive(Debug, FromQueryResult)]
struct AggregatedBarRow {
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
/// 既存行の OHLCV または adjustment_factor が異なる場合に更新する。
pub async fn upsert_bars(
    db: &impl sea_orm::ConnectionTrait,
    bars_data: Vec<Bar>,
) -> Result<(), DbErr> {
    if bars_data.is_empty() {
        return Ok(());
    }

    let mut coverage_by_series = std::collections::BTreeMap::<
        (String, String),
        (DateTime<FixedOffset>, DateTime<FixedOffset>),
    >::new();
    for bar in &bars_data {
        let key = (bar.instrument_id.clone(), bar.timeframe.to_string());
        let timestamp = bar.timestamp.fixed_offset();
        coverage_by_series
            .entry(key)
            .and_modify(|(earliest, latest)| {
                *earliest = (*earliest).min(timestamp);
                *latest = (*latest).max(timestamp);
            })
            .or_insert((timestamp, timestamp));
    }
    let coverage = coverage_by_series
        .into_iter()
        .map(
            |((instrument_id, timeframe), (earliest_timestamp, latest_timestamp))| {
                BarTimestampCoverage {
                    instrument_id,
                    timeframe,
                    earliest_timestamp,
                    latest_timestamp,
                }
            },
        )
        .collect::<Vec<_>>();

    // PostgreSQL の bind パラメータ数には u16::MAX の上限がある。
    let chunk_size = crate::repositories::max_rows_per_statement(bars::Column::iter().count());
    let active_models: Vec<bars::ActiveModel> = bars_data.into_iter().map(Into::into).collect();

    for chunk in active_models.chunks(chunk_size) {
        bars::Entity::insert_many(chunk.to_vec())
            .on_conflict(
                OnConflict::columns([
                    bars::Column::InstrumentId,
                    bars::Column::Timeframe,
                    bars::Column::Timestamp,
                ])
                .update_columns(BAR_UPDATE_COLUMNS)
                .action_cond_where(changed_bar_columns_condition())
                .to_owned(),
            )
            .exec_without_returning(db)
            .await?;
    }

    upsert_bar_timestamp_coverage(db, coverage).await?;

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

/// 1h は直近 12 か月を 1m から集計し、それより前は保存済み 1h を優先して欠損だけ 1m で補う。
pub async fn find_intraday_bars(
    db: &impl ConnectionTrait,
    query: BarsQuery,
    hourly_history_cutoff: Option<DateTime<FixedOffset>>,
) -> Result<Vec<Bar>, DbErr> {
    let Ok(timeframe) = query.timeframe.parse::<Timeframe>() else {
        return Ok(Vec::new());
    };

    find_intraday_bars_by_instruments(
        db,
        &[query.instrument_id],
        timeframe,
        hourly_history_cutoff,
        query.from,
        query.to,
    )
    .await
}

async fn find_bars_with_sql_template(
    db: &impl ConnectionTrait,
    sql_template: &str,
    interval: Option<&str>,
    timeframe: Timeframe,
    instrument_ids: &[String],
    from: Option<DateTime<FixedOffset>>,
    to: Option<DateTime<FixedOffset>>,
) -> Result<Vec<Bar>, DbErr> {
    if instrument_ids.is_empty() {
        return Ok(Vec::new());
    }

    let first_instrument_index = if interval.is_some() { 2 } else { 1 };
    let instrument_placeholders = (first_instrument_index
        ..first_instrument_index + instrument_ids.len())
        .map(|index| format!("${index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let from_index = first_instrument_index + instrument_ids.len();
    let to_index = from_index + 1;
    let sql = sql_template
        .replace("{ohlcv_aggregates}", OHLCV_AGGREGATES)
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
            let row = AggregatedBarRow::from_query_result(row, "")?;
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

/// 複数銘柄の intraday bars を取得する。1h は直近 12 か月を 1m から集計し、それより前は保存済み 1h を優先して欠損だけ 1m で補う。4h はその結果から集計する。
pub async fn find_intraday_bars_by_instruments(
    db: &impl ConnectionTrait,
    instrument_ids: &[String],
    timeframe: Timeframe,
    hourly_history_cutoff: Option<DateTime<FixedOffset>>,
    from: Option<DateTime<FixedOffset>>,
    to: Option<DateTime<FixedOffset>>,
) -> Result<Vec<Bar>, DbErr> {
    let (sql_template, interval) = match (timeframe, hourly_history_cutoff) {
        (Timeframe::Minute, _) => (FIND_ONE_MINUTE_BARS_SQL, None),
        (Timeframe::Hourly, Some(cutoff)) => {
            return us_stock_intraday::find_hourly_bars(db, instrument_ids, cutoff, from, to).await;
        }
        (Timeframe::FourHours, Some(cutoff)) => {
            return us_stock_intraday::find_four_hour_bars(db, instrument_ids, cutoff, from, to)
                .await;
        }
        (Timeframe::Hourly | Timeframe::FourHours, None) => {
            return Err(DbErr::Custom(
                "hourly history cutoff is required for intraday bars".to_owned(),
            ));
        }
        (timeframe, _) => match timeframe.bucket_interval() {
            Some(interval) => (FIND_AGGREGATED_MINUTE_BARS_SQL, Some(interval)),
            None => return Ok(Vec::new()),
        },
    };

    find_bars_with_sql_template(
        db,
        sql_template,
        interval,
        timeframe,
        instrument_ids,
        from,
        to,
    )
    .await
}

/// 日足から週足を集計して取得する。
pub async fn find_weekly_bars_by_instruments(
    db: &impl ConnectionTrait,
    instrument_ids: &[String],
    from: Option<DateTime<FixedOffset>>,
    to: Option<DateTime<FixedOffset>>,
) -> Result<Vec<Bar>, DbErr> {
    find_bars_with_sql_template(
        db,
        FIND_AGGREGATED_DAILY_BARS_SQL,
        Some("1 week"),
        Timeframe::Weekly,
        instrument_ids,
        from,
        to,
    )
    .await
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
    use chrono::{Duration as ChronoDuration, NaiveDate, TimeZone, Timelike, Utc};
    use core_application::bars::BarsRepository;
    use core_domain::bar::Timeframe;
    use rust_decimal::Decimal;
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{ConnectionTrait, DatabaseBackend, EntityTrait, FromQueryResult, Set, Statement};
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

    #[derive(Clone, Copy)]
    enum BarValue {
        Open,
        High,
        Low,
        Close,
        Volume,
        AdjustmentFactor,
    }

    fn correct_bar_value(mut bar: Bar, value: BarValue) -> Bar {
        match value {
            BarValue::Open => bar.open = Decimal::new(111, 0),
            BarValue::High => bar.high = Decimal::new(222, 0),
            BarValue::Low => bar.low = Decimal::new(33, 0),
            BarValue::Close => bar.close = Decimal::new(144, 0),
            BarValue::Volume => bar.volume = 4567,
            BarValue::AdjustmentFactor => bar.adjustment_factor = Decimal::new(125, 2),
        }
        bar
    }

    #[rstest::rstest]
    #[case::open(BarValue::Open)]
    #[case::high(BarValue::High)]
    #[case::low(BarValue::Low)]
    #[case::close(BarValue::Close)]
    #[case::volume(BarValue::Volume)]
    #[case::adjustment_factor(BarValue::AdjustmentFactor)]
    #[tokio::test]
    async fn upsert_bars_applies_each_correction(#[case] value: BarValue) {
        let db = crate::test_support::create_test_transaction().await;
        let instrument_id = "DEMO-BAR-CORRECTION";
        insert_test_instrument(&db, instrument_id).await;
        let date = NaiveDate::from_ymd_opt(2040, 1, 2).expect("valid fixture date");
        let initial = make_test_bar(instrument_id, date, 100);
        let corrected = correct_bar_value(initial.clone(), value);

        upsert_bars(&db, vec![initial]).await.expect("insert bar");
        upsert_bars(&db, vec![corrected.clone()])
            .await
            .expect("apply correction");

        let actual = find_bars(
            &db,
            BarsQuery {
                instrument_id: instrument_id.to_owned(),
                timeframe: "1d".to_owned(),
                from: None,
                to: None,
            },
        )
        .await
        .expect("find bar")
        .into_iter()
        .map(Into::into)
        .collect::<Vec<Bar>>();

        assert_eq!(actual, vec![corrected]);
    }

    #[derive(Debug, Eq, PartialEq, FromQueryResult)]
    struct BarUpdateCounts {
        bars: i64,
        coverage: i64,
    }

    #[derive(Debug, Eq, PartialEq, FromQueryResult)]
    struct BarChunkCompressionStatus {
        is_compressed: bool,
    }

    #[derive(Clone, Copy)]
    enum UnchangedBarCase {
        DailyUncompressed,
        MinuteCompressed,
    }

    impl UnchangedBarCase {
        fn is_compressed(self) -> bool {
            matches!(self, Self::MinuteCompressed)
        }
    }

    async fn find_bar_update_counts(db: &impl ConnectionTrait) -> BarUpdateCounts {
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT \
                    ( \
                        SELECT COALESCE(SUM(n_tup_upd), 0)::bigint \
                        FROM pg_stat_xact_all_tables \
                        WHERE relid IN ( \
                            SELECT format('%I.%I', chunk_schema, chunk_name)::regclass \
                            FROM timescaledb_information.chunks \
                            WHERE hypertable_schema = 'public' AND hypertable_name = 'bars' \
                        ) \
                    ) AS bars, \
                    COALESCE(( \
                        SELECT n_tup_upd \
                        FROM pg_stat_xact_all_tables \
                        WHERE relid = 'public.bars_timestamp_coverage'::regclass \
                    ), 0) AS coverage",
                [],
            ))
            .await
            .expect("query bar update counts")
            .expect("update counts exist");
        BarUpdateCounts::from_query_result(&row, "").expect("decode update counts")
    }

    async fn find_bar_chunk_compression_status(
        db: &impl ConnectionTrait,
        timestamp: DateTime<chrono::Utc>,
    ) -> bool {
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT is_compressed \
                FROM timescaledb_information.chunks \
                WHERE hypertable_schema = 'public' AND hypertable_name = 'bars' \
                  AND range_start <= $1 AND range_end > $1",
                [timestamp.fixed_offset().into()],
            ))
            .await
            .expect("read chunk compression status")
            .expect("chunk compression status exists");
        BarChunkCompressionStatus::from_query_result(&row, "")
            .expect("decode chunk compression status")
            .is_compressed
    }

    async fn prepare_unchanged_bar_case(
        db: &gateway_postgres::DatabaseHandle,
        instrument_id: &str,
        now: DateTime<Utc>,
        case: UnchangedBarCase,
    ) -> Bar {
        let bar = match case {
            UnchangedBarCase::DailyUncompressed => make_test_bar(
                instrument_id,
                NaiveDate::from_ymd_opt(2040, 1, 2).expect("valid fixture date"),
                100,
            ),
            UnchangedBarCase::MinuteCompressed => {
                let timestamp = truncate_to_microsecond_precision(now - ChronoDuration::days(90));
                make_test_minute_bar(instrument_id, timestamp, 100)
            }
        };
        upsert_bars(db, vec![bar.clone()])
            .await
            .expect("insert bar");

        match case {
            UnchangedBarCase::DailyUncompressed => {}
            UnchangedBarCase::MinuteCompressed => {
                crate::PostgresBarsRepository::new(db.clone())
                    .compress_bars_older_than_policy(now)
                    .await
                    .expect("compress old bar chunk");
            }
        }

        bar
    }

    fn truncate_to_microsecond_precision(timestamp: DateTime<Utc>) -> DateTime<Utc> {
        timestamp
            .with_nanosecond(timestamp.timestamp_subsec_micros() * 1_000)
            .expect("valid bar timestamp")
    }

    #[rstest::rstest]
    #[case::daily_uncompressed(UnchangedBarCase::DailyUncompressed)]
    #[case::minute_compressed(UnchangedBarCase::MinuteCompressed)]
    #[tokio::test]
    async fn upsert_bars_skips_unchanged_bar_and_coverage_updates(#[case] case: UnchangedBarCase) {
        let db = crate::test_support::create_test_transaction().await;
        let instrument_id = "DEMO-BAR-UNCHANGED";
        insert_test_instrument(&db, instrument_id).await;
        let now = Utc::now();
        let bar = prepare_unchanged_bar_case(&db, instrument_id, now, case).await;

        let compressed_before = find_bar_chunk_compression_status(&db, bar.timestamp).await;

        let update_counts_before = find_bar_update_counts(&db).await;

        upsert_bars(&db, vec![bar.clone()])
            .await
            .expect("upsert unchanged bar");

        let update_counts_after = find_bar_update_counts(&db).await;
        let compressed_after = find_bar_chunk_compression_status(&db, bar.timestamp).await;
        let actual = find_bars(
            &db,
            BarsQuery {
                instrument_id: instrument_id.to_owned(),
                timeframe: bar.timeframe.to_string(),
                from: Some(bar.timestamp.fixed_offset()),
                to: Some(bar.timestamp.fixed_offset()),
            },
        )
        .await
        .expect("find bar")
        .into_iter()
        .map(Into::into)
        .collect::<Vec<Bar>>();

        assert_eq!(
            (
                update_counts_after,
                actual,
                compressed_before,
                compressed_after,
            ),
            (
                update_counts_before,
                vec![bar],
                case.is_compressed(),
                case.is_compressed(),
            ),
        );
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
    async fn upsert_bars_inserts_more_than_ten_thousand_records(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let instrument_id = "TEST-BIND-LIMIT";
        insert_test_instrument(&db, instrument_id).await;

        let start_date = NaiveDate::from_ymd_opt(2040, 1, 1).expect("invalid date");
        let expected: Vec<Bar> = (0..10_001)
            .map(|day_offset| {
                let date = start_date
                    .checked_add_signed(chrono::Duration::days(day_offset))
                    .expect("invalid date offset");
                make_test_bar(instrument_id, date, day_offset)
            })
            .collect();

        upsert_bars(&db, expected.clone())
            .await
            .expect("upsert failed");

        let actual: Vec<Bar> = find_bars(
            &db,
            BarsQuery {
                instrument_id: instrument_id.to_owned(),
                timeframe: "1d".to_owned(),
                from: None,
                to: None,
            },
        )
        .await
        .expect("find failed")
        .into_iter()
        .map(Into::into)
        .collect();

        assert_eq!(actual, expected);
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
    async fn upsert_bars_updates_existing_minute_records(db: gateway_postgres::DatabaseHandle) {
        insert_test_instrument(&db, "US:QZ7").await;
        let timestamp = Utc
            .with_ymd_and_hms(2040, 1, 2, 15, 4, 0)
            .single()
            .expect("fixture timestamp");
        let updated_bar = make_test_minute_bar("US:QZ7", timestamp, 200);
        let mut hourly_bar = updated_bar.clone();
        hourly_bar.timeframe = Timeframe::Hourly;
        hourly_bar.high = Decimal::new(999, 0);

        upsert_bars(
            &db,
            vec![make_test_minute_bar("US:QZ7", timestamp, 100), hourly_bar],
        )
        .await
        .expect("insert bars");
        upsert_bars(&db, vec![updated_bar.clone()])
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
            None,
        )
        .await
        .expect("find minute bars");

        let mut expected_five_minute_bar = updated_bar.clone();
        expected_five_minute_bar.timeframe = Timeframe::FiveMinutes;
        expected_five_minute_bar.timestamp = Utc
            .with_ymd_and_hms(2040, 1, 2, 15, 0, 0)
            .single()
            .expect("five-minute bucket timestamp");
        let aggregated = find_intraday_bars(
            &db,
            BarsQuery {
                instrument_id: "US:QZ7".to_owned(),
                timeframe: "5m".to_owned(),
                from: None,
                to: None,
            },
            None,
        )
        .await
        .expect("find aggregated bars");

        assert_eq!(
            (result, aggregated),
            (vec![updated_bar], vec![expected_five_minute_bar]),
        );
    }

    #[backend_test_macros::database_test]
    async fn combines_hourly_bars_and_minute_fallback_for_hourly_and_four_hour_queries(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let instrument_id = format!("US:DEMO-MIX-{}", uuid::Uuid::new_v4().simple());
        insert_test_instrument(&db, &instrument_id).await;
        let bar = |timeframe, timestamp, open, high, low, close, volume| Bar {
            instrument_id: instrument_id.clone(),
            timeframe,
            timestamp,
            open: Decimal::new(open, 0),
            high: Decimal::new(high, 0),
            low: Decimal::new(low, 0),
            close: Decimal::new(close, 0),
            volume,
            adjustment_factor: Decimal::ONE,
        };
        let timestamp = |hour, minute| {
            Utc.with_ymd_and_hms(2040, 1, 2, hour, minute, 0)
                .single()
                .expect("fixture timestamp")
        };
        let bars = vec![
            bar(Timeframe::Hourly, timestamp(8, 0), 10, 15, 5, 12, 100),
            bar(Timeframe::Hourly, timestamp(9, 0), 12, 20, 11, 18, 200),
            bar(Timeframe::Hourly, timestamp(10, 0), 30, 900, 1, 800, 9_000),
            bar(
                Timeframe::Hourly,
                timestamp(11, 0),
                40,
                1_000,
                2,
                900,
                10_000,
            ),
            bar(
                Timeframe::Minute,
                timestamp(9, 30),
                800,
                999,
                700,
                950,
                5_000,
            ),
            bar(
                Timeframe::Minute,
                timestamp(10, 30),
                500,
                600,
                400,
                550,
                900,
            ),
            bar(
                Timeframe::Minute,
                timestamp(10, 59),
                550,
                650,
                450,
                600,
                900,
            ),
            bar(Timeframe::Minute, timestamp(11, 0), 22, 30, 21, 27, 50),
            bar(Timeframe::Minute, timestamp(11, 59), 27, 31, 25, 29, 60),
        ];
        upsert_bars(&db, bars).await.expect("insert fixture bars");

        let hourly = find_intraday_bars_by_instruments(
            &db,
            std::slice::from_ref(&instrument_id),
            Timeframe::Hourly,
            Some(timestamp(10, 0).fixed_offset()),
            Some(timestamp(8, 0).fixed_offset()),
            Some(timestamp(11, 0).fixed_offset()),
        )
        .await
        .expect("find hourly bars");
        let four_hour = find_intraday_bars_by_instruments(
            &db,
            std::slice::from_ref(&instrument_id),
            Timeframe::FourHours,
            Some(timestamp(10, 0).fixed_offset()),
            Some(timestamp(8, 0).fixed_offset()),
            Some(timestamp(8, 0).fixed_offset()),
        )
        .await
        .expect("find four-hour bars");

        assert_eq!(
            (hourly, four_hour),
            (
                vec![
                    bar(Timeframe::Hourly, timestamp(8, 0), 10, 15, 5, 12, 100),
                    bar(Timeframe::Hourly, timestamp(9, 0), 12, 20, 11, 18, 200),
                    bar(
                        Timeframe::Hourly,
                        timestamp(10, 0),
                        500,
                        650,
                        400,
                        600,
                        1800
                    ),
                    bar(Timeframe::Hourly, timestamp(11, 0), 22, 31, 21, 29, 110),
                ],
                vec![bar(
                    Timeframe::FourHours,
                    timestamp(8, 0),
                    10,
                    650,
                    5,
                    29,
                    2210,
                )],
            ),
        );
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
