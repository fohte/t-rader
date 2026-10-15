use async_trait::async_trait;
use core_application::group_movers::{
    GroupMover, GroupMoverStock, GroupMoversQuery, GroupMoversQueryError, GroupMoversQuerySource,
};
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

use crate::DatabaseHandle;
use crate::persistence::persistence_error;

const TOP_STOCKS_PER_GROUP: i64 = 5;

const LIST_GROUP_MOVERS_SQL: &str = r#"
    WITH axis_groups AS (
        SELECT group_row.id AS group_id,
               group_row.key AS group_key, group_row.name AS group_name
        FROM group_axis AS axis
        JOIN stock_group AS group_row ON group_row.axis_id = axis.id
        WHERE axis.key = $1::text
    ),
    members AS (
        SELECT axis_groups.group_id, axis_groups.group_key, axis_groups.group_name,
               member.stock_id
        FROM axis_groups
        LEFT JOIN stock_group_member AS member ON member.group_id = axis_groups.group_id
    ),
    period_bars AS (
        SELECT bars.instrument_id, bars.timestamp, bars.close, bars.volume
        FROM bars
        JOIN (SELECT DISTINCT stock_id FROM members WHERE stock_id IS NOT NULL) AS member_ids
          ON member_ids.stock_id = bars.instrument_id
        WHERE bars.timeframe = '1d'
          AND bars.timestamp >= ($2::date::timestamp AT TIME ZONE 'UTC')
          AND bars.timestamp < (($3::date + 1)::timestamp AT TIME ZONE 'UTC')
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
    group_returns AS (
        SELECT members.group_id, members.group_key, members.group_name, members.stock_id,
               COALESCE(stock.name, instruments.name) AS stock_name,
               end_prices.close / start_prices.close - 1 AS change_rate,
               avg_turnover.avg_turnover
        FROM members
        JOIN avg_turnover ON avg_turnover.instrument_id = members.stock_id
        JOIN end_prices ON end_prices.instrument_id = members.stock_id
        JOIN instruments ON instruments.id = members.stock_id
        LEFT JOIN stock ON stock.id = members.stock_id
        CROSS JOIN LATERAL (
            SELECT bars.close
            FROM bars
            WHERE bars.instrument_id = members.stock_id
              AND bars.timeframe = '1d'
              AND bars.timestamp < ($2::date::timestamp AT TIME ZONE 'UTC')
            ORDER BY bars.timestamp DESC
            LIMIT 1
        ) AS start_prices
        WHERE start_prices.close <> 0
    ),
    ranked_returns AS (
        SELECT group_returns.*,
               row_number() OVER (
                   PARTITION BY group_id
                   ORDER BY change_rate ASC, stock_id ASC
               ) AS return_rank,
               COUNT(*) OVER (PARTITION BY group_id) AS return_count,
               row_number() OVER (
                   PARTITION BY group_id
                   ORDER BY avg_turnover DESC, stock_id ASC
               ) AS stock_rank
        FROM group_returns
    ),
    group_metrics AS (
        SELECT axis_groups.group_id, axis_groups.group_key, axis_groups.group_name,
               COUNT(ranked_returns.stock_id)::bigint AS constituent_count,
               AVG(ranked_returns.change_rate) FILTER (
                   WHERE ranked_returns.return_rank IN (
                       (ranked_returns.return_count + 1) / 2,
                       (ranked_returns.return_count + 2) / 2
                   )
               ) AS median_change_rate,
               SUM(ranked_returns.change_rate * ranked_returns.avg_turnover)
                   / NULLIF(SUM(ranked_returns.avg_turnover), 0)
                   AS turnover_weighted_change_rate,
               COALESCE(SUM(ranked_returns.avg_turnover), 0::numeric) AS total_avg_turnover
        FROM axis_groups
        LEFT JOIN ranked_returns ON ranked_returns.group_id = axis_groups.group_id
        GROUP BY axis_groups.group_id, axis_groups.group_key, axis_groups.group_name
    )
    SELECT group_metrics.group_key, group_metrics.group_name,
           group_metrics.constituent_count, group_metrics.median_change_rate,
           group_metrics.turnover_weighted_change_rate, group_metrics.total_avg_turnover,
           ranked_returns.stock_id, ranked_returns.stock_name, ranked_returns.change_rate,
           ranked_returns.avg_turnover
    FROM group_metrics
    LEFT JOIN ranked_returns
      ON ranked_returns.group_id = group_metrics.group_id
     AND ranked_returns.stock_rank <= $5::bigint
    ORDER BY ABS(group_metrics.median_change_rate) DESC NULLS LAST,
             group_metrics.group_key ASC,
             ranked_returns.stock_rank ASC
"#;

#[derive(Debug, FromQueryResult)]
struct GroupMoverRow {
    group_key: String,
    group_name: String,
    constituent_count: i64,
    median_change_rate: Option<Decimal>,
    turnover_weighted_change_rate: Option<Decimal>,
    total_avg_turnover: Decimal,
    stock_id: Option<String>,
    stock_name: Option<String>,
    change_rate: Option<Decimal>,
    avg_turnover: Option<Decimal>,
}

#[derive(Clone)]
pub struct PostgresGroupMoversQuerySource {
    db: DatabaseHandle,
}

impl PostgresGroupMoversQuerySource {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl GroupMoversQuerySource for PostgresGroupMoversQuerySource {
    async fn list_group_movers(
        &self,
        query: GroupMoversQuery,
    ) -> Result<Vec<GroupMover>, GroupMoversQueryError> {
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                LIST_GROUP_MOVERS_SQL,
                [
                    query.axis_key.into(),
                    query.from.into(),
                    query.to.into(),
                    query.min_avg_turnover.into(),
                    TOP_STOCKS_PER_GROUP.into(),
                ],
            ))
            .await
            .map_err(query_error)?;

        let mut groups: Vec<GroupMover> = Vec::new();
        for row in &rows {
            let row = GroupMoverRow::from_query_result(row, "").map_err(query_error)?;
            if !matches!(groups.last(), Some(group) if group.group_key == row.group_key) {
                groups.push(GroupMover {
                    group_key: row.group_key,
                    name: row.group_name,
                    constituent_count: row.constituent_count,
                    median_change_rate: row.median_change_rate,
                    turnover_weighted_change_rate: row.turnover_weighted_change_rate,
                    total_avg_turnover: row.total_avg_turnover,
                    top_stocks: Vec::new(),
                });
            }

            if let Some(group) = groups.last_mut()
                && let (Some(stock_id), Some(name), Some(change_rate), Some(avg_turnover)) = (
                    row.stock_id,
                    row.stock_name,
                    row.change_rate,
                    row.avg_turnover,
                )
            {
                group.top_stocks.push(GroupMoverStock {
                    stock_id,
                    name,
                    change_rate,
                    avg_turnover,
                });
            }
        }

        Ok(groups)
    }
}

fn query_error(error: sea_orm::DbErr) -> GroupMoversQueryError {
    GroupMoversQueryError::Database(persistence_error(error))
}
