use chrono::{DateTime, FixedOffset};
use core_application::bars::BarsRepositoryError;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

use super::{PostgresBarsRepository, repository_error};

impl PostgresBarsRepository {
    pub(super) async fn find_us_stock_bar_targets(
        &self,
    ) -> Result<Vec<core_application::bars::UsStockBarTarget>, BarsRepositoryError> {
        #[derive(FromQueryResult)]
        struct TargetRow {
            instrument_id: String,
            latest_daily_bar: Option<DateTime<FixedOffset>>,
            latest_minute_bar: Option<DateTime<FixedOffset>>,
            latest_hourly_bar: Option<DateTime<FixedOffset>>,
            hourly_history_completed_through: Option<DateTime<FixedOffset>>,
            earliest_daily_bar: Option<DateTime<FixedOffset>>,
            earliest_minute_bar: Option<DateTime<FixedOffset>>,
            earliest_hourly_bar: Option<DateTime<FixedOffset>>,
        }

        let query = r#"SELECT
                instruments.id AS instrument_id,
                daily_coverage.latest_timestamp AS latest_daily_bar,
                minute_coverage.latest_timestamp AS latest_minute_bar,
                hourly_coverage.latest_timestamp AS latest_hourly_bar,
                hourly_progress.completed_through AS hourly_history_completed_through,
                daily_coverage.earliest_timestamp AS earliest_daily_bar,
                minute_coverage.earliest_timestamp AS earliest_minute_bar,
                hourly_coverage.earliest_timestamp AS earliest_hourly_bar
            FROM instruments
            LEFT JOIN bars_timestamp_coverage AS daily_coverage
                ON daily_coverage.instrument_id = instruments.id
               AND daily_coverage.timeframe = '1d'
            LEFT JOIN bars_timestamp_coverage AS minute_coverage
                ON minute_coverage.instrument_id = instruments.id
               AND minute_coverage.timeframe = '1m'
            LEFT JOIN bars_timestamp_coverage AS hourly_coverage
                ON hourly_coverage.instrument_id = instruments.id
               AND hourly_coverage.timeframe = '1h'
            LEFT JOIN us_stock_hourly_bar_ingest_progress AS hourly_progress
                ON hourly_progress.instrument_id = instruments.id
            WHERE instruments.market = 'US'
              AND (
                  EXISTS (
                      SELECT 1 FROM stock
                      WHERE stock.id = instruments.id
                        AND stock.market IN ('Nasdaq', 'NYSE')
                  )
                  OR EXISTS (
                      SELECT 1 FROM stock_group_member
                      WHERE stock_group_member.stock_id = instruments.id
                  )
                  OR EXISTS (
                      SELECT 1 FROM note_ref
                      WHERE note_ref.ref_kind = 'stock'
                        AND note_ref.ref_id = instruments.id
                  )
              )
            ORDER BY instruments.id"#
            .to_owned();
        let rows = self
            .db
            .query_all_raw(Statement::from_string(DatabaseBackend::Postgres, query))
            .await
            .map_err(repository_error)?;

        rows.iter()
            .map(|row| {
                let row = TargetRow::from_query_result(row, "").map_err(repository_error)?;
                Ok(core_application::bars::UsStockBarTarget {
                    instrument_id: row.instrument_id,
                    latest_daily_bar: row.latest_daily_bar.map(|timestamp| timestamp.to_utc()),
                    latest_minute_bar: row.latest_minute_bar.map(|timestamp| timestamp.to_utc()),
                    latest_hourly_bar: row.latest_hourly_bar.map(|timestamp| timestamp.to_utc()),
                    hourly_history_completed_through: row
                        .hourly_history_completed_through
                        .map(|timestamp| timestamp.to_utc()),
                    earliest_daily_bar: row.earliest_daily_bar.map(|timestamp| timestamp.to_utc()),
                    earliest_minute_bar: row
                        .earliest_minute_bar
                        .map(|timestamp| timestamp.to_utc()),
                    earliest_hourly_bar: row
                        .earliest_hourly_bar
                        .map(|timestamp| timestamp.to_utc()),
                })
            })
            .collect()
    }
}
