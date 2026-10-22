use chrono::{DateTime, FixedOffset, Utc};
use core_application::bars::UsStockMinuteBarsDownsampleStats;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

use crate::repositories::bars::{
    BarTimestampCoverage, OHLCV_AGGREGATES, upsert_bar_timestamp_coverage,
};

#[derive(Debug, FromQueryResult)]
struct TimestampRow {
    timestamp: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, FromQueryResult)]
struct EarliestTimestampRow {
    timestamp: DateTime<FixedOffset>,
}

#[derive(Debug, FromQueryResult)]
struct InsertedBarRow {
    timestamp: DateTime<FixedOffset>,
}

pub(super) async fn find_earliest_minute_bar_before(
    db: &impl ConnectionTrait,
    cutoff: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, sea_orm::DbErr> {
    // MIN/MAX 専用の順序探索を避け、chunk ごとの部分集約を全体でまとめる。
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT MIN(timestamp) AS timestamp FROM bars \
             WHERE instrument_id LIKE 'US:%' AND timeframe = '1m' AND timestamp < $1 \
             GROUP BY timeframe",
            [cutoff.fixed_offset().into()],
        ))
        .await?;

    row.map(|row| EarliestTimestampRow::from_query_result(&row, ""))
        .transpose()
        .map(|row| row.map(|row| row.timestamp.to_utc()))
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
        .query_all_raw(Statement::from_sql_and_values(
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
            RETURNING timestamp
            "#
            .replace("{ohlcv_aggregates}", OHLCV_AGGREGATES),
            values.clone(),
        ))
        .await?;

    let inserted_bar_count = u64::try_from(inserted.len())
        .map_err(|_| sea_orm::DbErr::Custom("inserted bar count exceeds u64".to_owned()))?;
    let mut hourly_bounds = None::<(DateTime<FixedOffset>, DateTime<FixedOffset>)>;
    for row in &inserted {
        let row = InsertedBarRow::from_query_result(row, "")?;
        hourly_bounds = Some(match hourly_bounds {
            Some((earliest, latest)) => (earliest.min(row.timestamp), latest.max(row.timestamp)),
            None => (row.timestamp, row.timestamp),
        });
    }
    if let Some((earliest_timestamp, latest_timestamp)) = hourly_bounds {
        upsert_bar_timestamp_coverage(
            transaction,
            vec![BarTimestampCoverage {
                instrument_id: instrument_id.to_owned(),
                timeframe: "1h".to_owned(),
                earliest_timestamp,
                latest_timestamp,
            }],
        )
        .await?;
    }

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
    let deleted_count = deleted.rows_affected();

    if deleted_count > 0 {
        let earliest_timestamp = transaction
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
                SELECT MIN(timestamp) AS timestamp
                FROM bars
                WHERE instrument_id = $1
                  AND timeframe = '1m'
                  AND timestamp >= LEAST($2::timestamptz, time_bucket('1 hour', $3::timestamptz))
                "#,
                [
                    instrument_id.to_owned().into(),
                    until.fixed_offset().into(),
                    cutoff.fixed_offset().into(),
                ],
            ))
            .await?
            .map(|row| TimestampRow::from_query_result(&row, ""))
            .transpose()?
            .and_then(|row| row.timestamp);

        match earliest_timestamp {
            Some(earliest_timestamp) => {
                transaction
                    .execute_raw(Statement::from_sql_and_values(
                        DatabaseBackend::Postgres,
                        "UPDATE bars_timestamp_coverage SET earliest_timestamp = $2 \
                         WHERE instrument_id = $1 AND timeframe = '1m'",
                        [instrument_id.to_owned().into(), earliest_timestamp.into()],
                    ))
                    .await?;
            }
            None => {
                transaction
                    .execute_raw(Statement::from_sql_and_values(
                        DatabaseBackend::Postgres,
                        "DELETE FROM bars_timestamp_coverage \
                         WHERE instrument_id = $1 AND timeframe = '1m'",
                        [instrument_id.to_owned().into()],
                    ))
                    .await?;
            }
        }
    }

    Ok(UsStockMinuteBarsDownsampleStats {
        hourly_bars_inserted: inserted_bar_count,
        minute_bars_deleted: deleted_count,
    })
}
