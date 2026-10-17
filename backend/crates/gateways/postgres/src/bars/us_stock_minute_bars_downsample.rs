use chrono::{DateTime, FixedOffset, Utc};
use core_application::bars::UsStockMinuteBarsDownsampleStats;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

use crate::repositories::bars::OHLCV_AGGREGATES;

#[derive(Debug, FromQueryResult)]
struct TimestampRow {
    timestamp: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, FromQueryResult)]
struct InstrumentIdRow {
    instrument_id: String,
}

pub(super) async fn find_earliest_minute_bar_before(
    db: &impl ConnectionTrait,
    cutoff: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT MIN(timestamp) AS timestamp FROM bars \
             WHERE instrument_id LIKE 'US:%' AND timeframe = '1m' AND timestamp < $1",
            [cutoff.fixed_offset().into()],
        ))
        .await?;

    row.map(|row| TimestampRow::from_query_result(&row, ""))
        .transpose()
        .map(|row| row.and_then(|row| row.timestamp.map(|timestamp| timestamp.to_utc())))
}

pub(super) async fn find_minute_bar_instruments(
    db: &impl ConnectionTrait,
    from: DateTime<Utc>,
    until: DateTime<Utc>,
    cutoff: DateTime<Utc>,
) -> Result<Vec<String>, sea_orm::DbErr> {
    db.query_all_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT DISTINCT instrument_id FROM bars \
         WHERE instrument_id LIKE 'US:%' AND timeframe = '1m' \
           AND timestamp >= $1 AND timestamp < $2 AND timestamp < $3 \
         ORDER BY instrument_id",
        [
            from.fixed_offset().into(),
            until.fixed_offset().into(),
            cutoff.fixed_offset().into(),
        ],
    ))
    .await?
    .iter()
    .map(|row| InstrumentIdRow::from_query_result(row, "").map(|row| row.instrument_id))
    .collect()
}

pub(super) async fn downsample_minute_bar_range(
    transaction: &sea_orm::DatabaseTransaction,
    instrument_id: &str,
    from: DateTime<Utc>,
    until: DateTime<Utc>,
    cutoff: DateTime<Utc>,
) -> Result<UsStockMinuteBarsDownsampleStats, sea_orm::DbErr> {
    let values = [
        from.fixed_offset().into(),
        until.fixed_offset().into(),
        cutoff.fixed_offset().into(),
        instrument_id.to_owned().into(),
    ];
    let inserted = transaction
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            WITH aggregated AS (
                SELECT
                    instrument_id,
                    time_bucket('1 hour', timestamp) AS timestamp,
                    {ohlcv_aggregates}
                FROM bars
                WHERE instrument_id LIKE 'US:%'
                  AND timeframe = '1m'
                  AND timestamp >= $1
                  AND timestamp < $2
                  AND timestamp < $3
                  AND instrument_id = $4
                GROUP BY instrument_id, time_bucket('1 hour', timestamp)
            )
            INSERT INTO bars (instrument_id, timeframe, timestamp, open, high, low, close, volume, adjustment_factor)
            SELECT aggregated.instrument_id, '1h', aggregated.timestamp,
                   aggregated.open, aggregated.high, aggregated.low,
                   aggregated.close, aggregated.volume, 1
            FROM aggregated
            WHERE aggregated.timestamp + INTERVAL '1 hour' <= $3
               OR EXISTS (
                    SELECT 1 FROM bars AS existing_hourly
                    WHERE existing_hourly.instrument_id = aggregated.instrument_id
                      AND existing_hourly.timeframe = '1h'
                      AND existing_hourly.timestamp = aggregated.timestamp
                      AND existing_hourly.timestamp >= time_bucket('1 hour', $1::timestamptz)
                      AND existing_hourly.timestamp < $2
                      AND existing_hourly.timestamp < $3
                      AND existing_hourly.instrument_id = $4
               )
            ON CONFLICT (instrument_id, timeframe, timestamp) DO NOTHING
            "#
            .replace("{ohlcv_aggregates}", OHLCV_AGGREGATES),
            values.clone(),
        ))
        .await?;

    // 1 時間足が保存されていない分足を残し、次回の実行で再処理できるようにする。
    let deleted = transaction
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            DELETE FROM bars AS minute
            WHERE minute.instrument_id LIKE 'US:%'
              AND minute.timeframe = '1m'
              AND minute.timestamp >= $1
              AND minute.timestamp < $2
              AND minute.timestamp < $3
              AND minute.instrument_id = $4
              AND EXISTS (
                    SELECT 1 FROM bars AS hourly
                    WHERE hourly.instrument_id = minute.instrument_id
                      AND hourly.timeframe = '1h'
                      AND hourly.timestamp = time_bucket('1 hour', minute.timestamp)
                      AND hourly.timestamp >= time_bucket('1 hour', $1::timestamptz)
                      AND hourly.timestamp < $2
                      AND hourly.timestamp < $3
                      AND hourly.instrument_id = $4
              )
            "#,
            values,
        ))
        .await?;

    Ok(UsStockMinuteBarsDownsampleStats {
        hourly_bars_inserted: inserted.rows_affected(),
        minute_bars_deleted: deleted.rows_affected(),
    })
}
