use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use core_application::market_movers::{
    MarketMover, MarketMoversQuery, MarketMoversQueryError, MarketMoversQuerySource,
};
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

use crate::DatabaseHandle;
use crate::persistence::persistence_error;

const LIST_MOVERS_SQL: &str = r#"
    WITH period_bars AS (
        SELECT instrument_id, timestamp, close, volume
        FROM bars
        WHERE timeframe = '1d'
          AND timestamp >= ($2::date::timestamp AT TIME ZONE 'UTC')
          AND timestamp < (($3::date + 1)::timestamp AT TIME ZONE 'UTC')
    ),
    avg_turnover AS (
        SELECT instrument_id, AVG(close * volume::numeric) AS avg_turnover
        FROM period_bars
        GROUP BY instrument_id
        HAVING AVG(close * volume::numeric) >= $4::numeric
    ),
    end_prices AS (
        SELECT DISTINCT ON (instrument_id) instrument_id, close
        FROM period_bars
        ORDER BY instrument_id, timestamp DESC
    ),
    start_prices AS (
        SELECT candidates.instrument_id, previous_bar.close
        FROM avg_turnover AS candidates
        CROSS JOIN LATERAL (
            SELECT bars.close
            FROM bars
            WHERE bars.instrument_id = candidates.instrument_id
              AND bars.timeframe = '1d'
              AND bars.timestamp < ($2::date::timestamp AT TIME ZONE 'UTC')
            ORDER BY bars.timestamp DESC
            LIMIT 1
        ) AS previous_bar
    ),
    activity AS (
        SELECT evidence.source_ref AS instrument_id, evidence.observed_at AS seen_at,
               'query_data'::text AS seen_via
        FROM strategy_task_step_evidence AS evidence
        JOIN strategy_task_step AS step
          ON step.execution_step_id = evidence.execution_step_id
        JOIN strategy_task AS task
          ON task.task_id = step.task_id
        WHERE task.strategy_id = $1::uuid
          AND evidence.source = 'query_data'
          AND evidence.observed_at >= ($2::date::timestamp AT TIME ZONE 'UTC')
          AND evidence.observed_at < (($3::date + 1)::timestamp AT TIME ZONE 'UTC')

        UNION ALL

        SELECT ref.ref_id AS instrument_id, version.created_at AS seen_at,
               'note'::text AS seen_via
        FROM note_ref AS ref
        JOIN note_version AS version
          ON version.note_id = ref.note_id
        JOIN strategy_task_step AS step
          ON version.execution_id = step.execution_step_id::text
        JOIN strategy_task AS task
          ON task.task_id = step.task_id
        WHERE task.strategy_id = $1::uuid
          AND ref.ref_kind = 'stock'
          AND version.created_at >= ($2::date::timestamp AT TIME ZONE 'UTC')
          AND version.created_at < (($3::date + 1)::timestamp AT TIME ZONE 'UTC')

        UNION ALL

        SELECT trade.symbol AS instrument_id,
               trade.date::timestamp AT TIME ZONE 'UTC' AS seen_at,
               'trade'::text AS seen_via
        FROM trade
        WHERE trade.strategy_id = $1::uuid
          AND trade.date >= $2::date
          AND trade.date <= $3::date

        UNION ALL

        SELECT order_row.stock_id AS instrument_id, order_row.ordered_at AS seen_at,
               'paper_trade'::text AS seen_via
        FROM paper_order AS order_row
        JOIN paper_account AS account
          ON account.id = order_row.account_id
        WHERE account.strategy_id = $1::uuid
          AND order_row.ordered_at >= ($2::date::timestamp AT TIME ZONE 'UTC')
          AND order_row.ordered_at < (($3::date + 1)::timestamp AT TIME ZONE 'UTC')

        UNION ALL

        SELECT prediction.target_stock_id AS instrument_id,
               prediction.created_at AS seen_at,
               'prediction'::text AS seen_via
        FROM prediction
        WHERE prediction.strategy_id = $1::uuid
          AND prediction.created_at >= ($2::date::timestamp AT TIME ZONE 'UTC')
          AND prediction.created_at < (($3::date + 1)::timestamp AT TIME ZONE 'UTC')
    ),
    first_seen AS (
        SELECT DISTINCT ON (instrument_id) instrument_id, seen_at, seen_via
        FROM activity
        ORDER BY instrument_id, seen_at, seen_via
    ),
    movers AS (
        SELECT end_prices.instrument_id,
               end_prices.close / start_prices.close - 1 AS change_rate,
               avg_turnover.avg_turnover
        FROM end_prices
        JOIN start_prices USING (instrument_id)
        JOIN avg_turnover USING (instrument_id)
        WHERE start_prices.close <> 0
    )
    SELECT movers.instrument_id, instruments.name, movers.change_rate,
           movers.avg_turnover, first_seen.seen_at AS first_seen_at,
           first_seen.seen_via
    FROM movers
    JOIN instruments
      ON instruments.id = movers.instrument_id
    LEFT JOIN first_seen
      ON first_seen.instrument_id = movers.instrument_id
    WHERE $5::text = 'abs'
       OR ($5::text = 'up' AND movers.change_rate > 0)
       OR ($5::text = 'down' AND movers.change_rate < 0)
    ORDER BY
        CASE WHEN $5::text = 'up' THEN movers.change_rate END DESC,
        CASE WHEN $5::text = 'down' THEN movers.change_rate END ASC,
        CASE WHEN $5::text = 'abs' THEN ABS(movers.change_rate) END DESC,
        movers.instrument_id ASC
    LIMIT $6
"#;

#[derive(Debug, FromQueryResult)]
struct MarketMoverRow {
    instrument_id: String,
    name: String,
    change_rate: Decimal,
    avg_turnover: Decimal,
    first_seen_at: Option<DateTime<FixedOffset>>,
    seen_via: Option<String>,
}

#[derive(Clone)]
pub struct PostgresMarketMoversQuerySource {
    db: DatabaseHandle,
}

impl PostgresMarketMoversQuerySource {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MarketMoversQuerySource for PostgresMarketMoversQuerySource {
    async fn list_movers(
        &self,
        strategy_id: uuid::Uuid,
        query: MarketMoversQuery,
    ) -> Result<Vec<MarketMover>, MarketMoversQueryError> {
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                LIST_MOVERS_SQL,
                [
                    strategy_id.into(),
                    query.from.into(),
                    query.to.into(),
                    query.min_avg_turnover.into(),
                    query.direction.as_str().into(),
                    i64::from(query.limit).into(),
                ],
            ))
            .await
            .map_err(query_error)?;

        rows.iter()
            .map(|row| {
                MarketMoverRow::from_query_result(row, "")
                    .map(|row| MarketMover {
                        instrument_id: row.instrument_id,
                        name: row.name,
                        change_rate: row.change_rate,
                        avg_turnover: row.avg_turnover,
                        first_seen_at: row.first_seen_at,
                        seen_via: row.seen_via,
                    })
                    .map_err(query_error)
            })
            .collect()
    }
}

fn query_error(error: sea_orm::DbErr) -> MarketMoversQueryError {
    MarketMoversQueryError::Database(persistence_error(error))
}
