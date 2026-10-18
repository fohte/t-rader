use std::sync::LazyLock;

use async_trait::async_trait;
use core_application::group_movers::{
    GroupMover, GroupMoverStock, GroupMoversQuery, GroupMoversQueryError, GroupMoversQuerySource,
};
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

use crate::DatabaseHandle;
use crate::group_query_sql::with_shared_market_ctes;
use crate::persistence::persistence_error;

const TOP_STOCKS_PER_GROUP: i64 = 5;

const GROUP_MOVER_STOCK_CHANGE_CTES: &str = r#"
    ranked_period_bars AS (
        SELECT period_bars.stock_id,
               period_bars.close,
               period_bars.is_end_period,
               ROW_NUMBER() OVER (
                   PARTITION BY period_bars.stock_id, period_bars.is_end_period
                   ORDER BY period_bars.timestamp DESC
               ) AS bar_rank
        FROM period_bars
    ),
    stock_change_prices AS (
        SELECT ranked_period_bars.stock_id,
               MAX(ranked_period_bars.close) FILTER (
                   WHERE ranked_period_bars.is_end_period
                     AND ranked_period_bars.bar_rank = 1
               ) AS end_close,
               MAX(ranked_period_bars.close) FILTER (
                   WHERE NOT ranked_period_bars.is_end_period
                     AND ranked_period_bars.bar_rank = 1
               ) AS start_close
        FROM ranked_period_bars
        GROUP BY ranked_period_bars.stock_id
    ),
    stock_changes AS (
        SELECT stock_change_prices.stock_id,
               stock_change_prices.end_close
                   / NULLIF(stock_change_prices.start_close, 0) - 1 AS change_rate
        FROM stock_change_prices
        WHERE stock_change_prices.end_close IS NOT NULL
          AND stock_change_prices.start_close IS NOT NULL
          AND stock_change_prices.start_close <> 0
    )
"#;

const GROUP_AXIS_EXISTS_SQL: &str =
    "SELECT EXISTS (SELECT 1 FROM group_axis WHERE key = $1::text) AS axis_exists";

static LIST_GROUP_MOVERS_SQL: LazyLock<String> = LazyLock::new(|| {
    with_shared_market_ctes(
        "SELECT $1::text AS axis_key",
        r#"
            eligible_stocks AS (
                SELECT DISTINCT group_members.stock_id
                FROM group_members
                WHERE group_members.axis_key = $1::text
                  AND group_members.stock_id IS NOT NULL
            ),
            period_bars AS MATERIALIZED (
                SELECT eligible_stocks.stock_id,
                       bars.timestamp,
                       bars.close,
                       bars.volume,
                       bars.timestamp >= ($2::date::timestamp AT TIME ZONE 'UTC')
                           AS is_end_period
                FROM eligible_stocks
                JOIN bars ON bars.instrument_id = eligible_stocks.stock_id
                WHERE bars.timeframe = '1d'
                  AND bars.timestamp >= (($2::date - 14)::timestamp AT TIME ZONE 'UTC')
                  AND bars.timestamp < (($3::date + 1)::timestamp AT TIME ZONE 'UTC')
            )
        "#,
        GROUP_MOVER_STOCK_CHANGE_CTES,
        r#"
            avg_turnover AS MATERIALIZED (
                SELECT period_bars.stock_id AS instrument_id,
                       AVG(period_bars.close * period_bars.volume::numeric) AS avg_turnover
                FROM period_bars
                WHERE period_bars.is_end_period
                GROUP BY period_bars.stock_id
                HAVING AVG(period_bars.close * period_bars.volume::numeric) >= $4::numeric
            ),
            group_returns AS (
                SELECT group_members.group_id, group_members.group_key,
                       group_members.group_name, group_members.stock_id,
                       COALESCE(stock.name, instruments.name) AS stock_name,
                       stock_changes.change_rate, avg_turnover.avg_turnover
                FROM group_members
                JOIN avg_turnover
                  ON avg_turnover.instrument_id = group_members.stock_id
                JOIN stock_changes
                  ON stock_changes.stock_id = group_members.stock_id
                JOIN instruments ON instruments.id = group_members.stock_id
                LEFT JOIN stock ON stock.id = group_members.stock_id
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
                SELECT axis_groups.group_id, axis_groups.group_key,
                       axis_groups.group_name,
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
                       COALESCE(SUM(ranked_returns.avg_turnover), 0::numeric)
                           AS total_avg_turnover
                FROM axis_groups
                LEFT JOIN ranked_returns ON ranked_returns.group_id = axis_groups.group_id
                GROUP BY axis_groups.group_id, axis_groups.group_key,
                         axis_groups.group_name
            )
            SELECT group_metrics.group_key, group_metrics.group_name,
                   group_metrics.constituent_count, group_metrics.median_change_rate,
                   group_metrics.turnover_weighted_change_rate,
                   group_metrics.total_avg_turnover,
                   ranked_returns.stock_id, ranked_returns.stock_name,
                   ranked_returns.change_rate, ranked_returns.avg_turnover
            FROM group_metrics
            LEFT JOIN ranked_returns
              ON ranked_returns.group_id = group_metrics.group_id
             AND ranked_returns.stock_rank <= $5::bigint
            ORDER BY ABS(group_metrics.median_change_rate) DESC NULLS LAST,
                     group_metrics.group_key ASC,
                     ranked_returns.stock_rank ASC
        "#,
    )
});

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
    async fn axis_exists(&self, axis_key: &str) -> Result<bool, GroupMoversQueryError> {
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                GROUP_AXIS_EXISTS_SQL,
                [axis_key.into()],
            ))
            .await
            .map_err(query_error)?;
        let Some(row) = row else {
            return Ok(false);
        };

        row.try_get("", "axis_exists").map_err(query_error)
    }

    async fn list_group_movers(
        &self,
        query: GroupMoversQuery,
    ) -> Result<Vec<GroupMover>, GroupMoversQueryError> {
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                LIST_GROUP_MOVERS_SQL.as_str(),
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
