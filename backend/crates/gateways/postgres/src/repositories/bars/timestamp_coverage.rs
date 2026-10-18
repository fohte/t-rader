use chrono::{DateTime, FixedOffset};
use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::{DbErr, EntityTrait, Iterable, Set};

use crate::entities::bars_timestamp_coverage;

pub(crate) struct BarTimestampCoverage {
    pub instrument_id: String,
    pub timeframe: String,
    pub earliest_timestamp: DateTime<FixedOffset>,
    pub latest_timestamp: DateTime<FixedOffset>,
}

pub(crate) async fn upsert_bar_timestamp_coverage(
    db: &impl sea_orm::ConnectionTrait,
    coverage: Vec<BarTimestampCoverage>,
) -> Result<(), DbErr> {
    if coverage.is_empty() {
        return Ok(());
    }

    let chunk_size = crate::repositories::max_rows_per_statement(
        bars_timestamp_coverage::Column::iter().count(),
    );
    for chunk in coverage.chunks(chunk_size) {
        bars_timestamp_coverage::Entity::insert_many(chunk.iter().map(|coverage| {
            bars_timestamp_coverage::ActiveModel {
                instrument_id: Set(coverage.instrument_id.clone()),
                timeframe: Set(coverage.timeframe.clone()),
                earliest_timestamp: Set(coverage.earliest_timestamp),
                latest_timestamp: Set(coverage.latest_timestamp),
            }
        }))
        .on_conflict(
            OnConflict::columns([
                bars_timestamp_coverage::Column::InstrumentId,
                bars_timestamp_coverage::Column::Timeframe,
            ])
            .value(
                bars_timestamp_coverage::Column::EarliestTimestamp,
                Expr::cust(
                    "LEAST(bars_timestamp_coverage.earliest_timestamp, EXCLUDED.earliest_timestamp)",
                ),
            )
            .value(
                bars_timestamp_coverage::Column::LatestTimestamp,
                Expr::cust(
                    "GREATEST(bars_timestamp_coverage.latest_timestamp, EXCLUDED.latest_timestamp)",
                ),
            )
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
    }

    Ok(())
}
