use chrono::{DateTime, FixedOffset};
use core_domain::bar::{Bar, Timeframe};
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

use super::OHLCV_AGGREGATES;

const FIND_HOURLY_BARS_SQL: &str = r#"
    WITH minute_bars AS (
        SELECT
            instrument_id,
            time_bucket('1 hour', timestamp) AS timestamp,
            {ohlcv_aggregates}
        FROM bars
        WHERE instrument_id IN ({instrument_ids})
          AND timeframe = '1m'
          AND (${from}::timestamptz IS NULL OR timestamp >= ${from})
          AND (${to}::timestamptz IS NULL OR timestamp < ${to} + INTERVAL '1 hour')
        GROUP BY instrument_id, time_bucket('1 hour', timestamp)
    ), source_bars AS (
        SELECT instrument_id, timestamp, open, high, low, close, volume
        FROM bars
        WHERE instrument_id IN ({instrument_ids})
          AND timeframe = '1h'
          AND timestamp < $1
          AND (${from}::timestamptz IS NULL OR timestamp >= ${from})
          AND (${to}::timestamptz IS NULL OR timestamp <= ${to})
        UNION ALL
        SELECT instrument_id, timestamp, open, high, low, close, volume
        FROM minute_bars
        WHERE NOT EXISTS (
            SELECT 1
            FROM bars AS hourly
            WHERE hourly.instrument_id = minute_bars.instrument_id
              AND hourly.timeframe = '1h'
              AND hourly.timestamp = minute_bars.timestamp
              AND hourly.timestamp < $1
              AND (${from}::timestamptz IS NULL OR hourly.timestamp >= ${from})
              AND (${to}::timestamptz IS NULL OR hourly.timestamp <= ${to})
        )
    )
    SELECT instrument_id, timestamp, open, high, low, close, volume
    FROM source_bars
    WHERE (${from}::timestamptz IS NULL OR timestamp >= ${from})
      AND (${to}::timestamptz IS NULL OR timestamp <= ${to})
    ORDER BY instrument_id ASC, timestamp ASC
"#;

const FIND_FOUR_HOUR_BARS_SQL: &str = r#"
    WITH minute_bars AS (
        SELECT
            instrument_id,
            time_bucket('1 hour', timestamp) AS timestamp,
            {ohlcv_aggregates}
        FROM bars
        WHERE instrument_id IN ({instrument_ids})
          AND timeframe = '1m'
          AND (${from}::timestamptz IS NULL OR timestamp >= ${from})
          AND (${to}::timestamptz IS NULL OR timestamp < ${to} + INTERVAL '4 hours')
        GROUP BY instrument_id, time_bucket('1 hour', timestamp)
    ), source_hours AS (
        SELECT instrument_id, timestamp, open, high, low, close, volume
        FROM bars
        WHERE instrument_id IN ({instrument_ids})
          AND timeframe = '1h'
          AND timestamp < $1
          AND (${from}::timestamptz IS NULL OR timestamp >= ${from})
          AND (${to}::timestamptz IS NULL OR timestamp < ${to} + INTERVAL '4 hours')
        UNION ALL
        SELECT instrument_id, timestamp, open, high, low, close, volume
        FROM minute_bars
        WHERE NOT EXISTS (
            SELECT 1
            FROM bars AS hourly
            WHERE hourly.instrument_id = minute_bars.instrument_id
              AND hourly.timeframe = '1h'
              AND hourly.timestamp = minute_bars.timestamp
              AND hourly.timestamp < $1
              AND (${from}::timestamptz IS NULL OR hourly.timestamp >= ${from})
              AND (${to}::timestamptz IS NULL OR hourly.timestamp < ${to} + INTERVAL '4 hours')
        )
    ), aggregated AS (
        SELECT
            instrument_id,
            time_bucket('4 hours', timestamp) AS timestamp,
            {ohlcv_aggregates}
        FROM source_hours
        GROUP BY instrument_id, time_bucket('4 hours', timestamp)
    )
    SELECT instrument_id, timestamp, open, high, low, close, volume
    FROM aggregated
    WHERE (${from}::timestamptz IS NULL OR timestamp >= ${from})
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

pub(super) async fn find_hourly_bars(
    db: &impl ConnectionTrait,
    instrument_ids: &[String],
    cutoff: DateTime<FixedOffset>,
    from: Option<DateTime<FixedOffset>>,
    to: Option<DateTime<FixedOffset>>,
) -> Result<Vec<Bar>, sea_orm::DbErr> {
    query_bars(
        db,
        FIND_HOURLY_BARS_SQL,
        Timeframe::Hourly,
        instrument_ids,
        cutoff,
        from,
        to,
    )
    .await
}

pub(super) async fn find_four_hour_bars(
    db: &impl ConnectionTrait,
    instrument_ids: &[String],
    cutoff: DateTime<FixedOffset>,
    from: Option<DateTime<FixedOffset>>,
    to: Option<DateTime<FixedOffset>>,
) -> Result<Vec<Bar>, sea_orm::DbErr> {
    query_bars(
        db,
        FIND_FOUR_HOUR_BARS_SQL,
        Timeframe::FourHours,
        instrument_ids,
        cutoff,
        from,
        to,
    )
    .await
}

async fn query_bars(
    db: &impl ConnectionTrait,
    sql_template: &str,
    timeframe: Timeframe,
    instrument_ids: &[String],
    cutoff: DateTime<FixedOffset>,
    from: Option<DateTime<FixedOffset>>,
    to: Option<DateTime<FixedOffset>>,
) -> Result<Vec<Bar>, sea_orm::DbErr> {
    if instrument_ids.is_empty() {
        return Ok(Vec::new());
    }

    let first_instrument_index = 2;
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
    values.push(cutoff.into());
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
