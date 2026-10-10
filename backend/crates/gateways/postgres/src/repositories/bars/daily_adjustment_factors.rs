use chrono::NaiveDate;
use core_application::bars::DailyBarAdjustmentFactor;
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

const FIND_DAILY_ADJUSTMENT_FACTORS_FROM_SQL: &str = "SELECT (timestamp AT TIME ZONE 'UTC')::date AS date, adjustment_factor AS factor FROM bars WHERE instrument_id = $1 AND timeframe = '1d' AND timestamp >= ($2::date::timestamp AT TIME ZONE 'UTC') AND adjustment_factor <> 1 ORDER BY timestamp ASC";

#[derive(Debug, FromQueryResult)]
struct DailyAdjustmentFactorRow {
    date: NaiveDate,
    factor: Decimal,
}

impl From<DailyAdjustmentFactorRow> for DailyBarAdjustmentFactor {
    fn from(row: DailyAdjustmentFactorRow) -> Self {
        Self {
            date: row.date,
            factor: row.factor,
        }
    }
}

pub async fn find_daily_adjustment_factors_from(
    db: &impl ConnectionTrait,
    instrument_id: &str,
    from: NaiveDate,
) -> Result<Vec<DailyBarAdjustmentFactor>, sea_orm::DbErr> {
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            FIND_DAILY_ADJUSTMENT_FACTORS_FROM_SQL,
            [instrument_id.to_owned().into(), from.into()],
        ))
        .await?;

    rows.into_iter()
        .map(|row| DailyAdjustmentFactorRow::from_query_result(&row, "").map(Into::into))
        .collect()
}
