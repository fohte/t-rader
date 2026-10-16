use chrono::{DateTime, FixedOffset, Utc};
use core_application::bars::UsStockMinuteBarsDownsampleStats;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

#[derive(Debug, FromQueryResult)]
struct TimestampRow {
    timestamp: Option<DateTime<FixedOffset>>,
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

pub(super) async fn downsample_minute_bar_range(
    transaction: &sea_orm::DatabaseTransaction,
    from: DateTime<Utc>,
    until: DateTime<Utc>,
    cutoff: DateTime<Utc>,
) -> Result<UsStockMinuteBarsDownsampleStats, sea_orm::DbErr> {
    let values = [
        from.fixed_offset().into(),
        until.fixed_offset().into(),
        cutoff.fixed_offset().into(),
    ];
    let inserted = transaction
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            WITH aggregated AS (
                SELECT
                    instrument_id,
                    time_bucket('1 hour', timestamp) AS timestamp,
                    first(open, timestamp) AS open,
                    MAX(high) AS high,
                    MIN(low) AS low,
                    last(close, timestamp) AS close,
                    SUM(volume)::bigint AS volume
                FROM bars
                WHERE instrument_id LIKE 'US:%'
                  AND timeframe = '1m'
                  AND timestamp >= $1
                  AND timestamp < $2
                  AND timestamp < $3
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
               )
            ON CONFLICT (instrument_id, timeframe, timestamp) DO NOTHING
            "#,
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
              AND EXISTS (
                    SELECT 1 FROM bars AS hourly
                    WHERE hourly.instrument_id = minute.instrument_id
                      AND hourly.timeframe = '1h'
                      AND hourly.timestamp = time_bucket('1 hour', minute.timestamp)
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
