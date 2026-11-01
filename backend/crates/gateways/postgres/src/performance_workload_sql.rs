/// 必須 workload が通る gateway SQL を PostgreSQL の観測ログと照合する。
pub fn statements() -> Vec<(&'static str, &'static str)> {
    let mut statements = crate::widget_query::performance_workload_sql();
    statements.extend(crate::group_movers::performance_workload_sql());
    statements
}

/// 最適化前の group movers SQL を検出器の対照として返す。
pub fn unoptimized_group_movers_sql() -> String {
    crate::group_query_sql::with_shared_market_ctes(
        "SELECT $1::text AS axis_key",
        r#"
            eligible_stocks AS (
                SELECT DISTINCT stock_group_member.stock_id
                FROM axis_groups
                JOIN stock_group_member
                  ON stock_group_member.group_id = axis_groups.group_id
                WHERE stock_group_member.stock_id IS NOT NULL
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
        r#"
            latest_period_bars AS (
                SELECT period_bars.stock_id,
                       period_bars.is_end_period,
                       MAX(period_bars.timestamp) AS timestamp
                FROM period_bars
                GROUP BY period_bars.stock_id, period_bars.is_end_period
            ),
            stock_change_prices AS (
                SELECT latest_period_bars.stock_id,
                       MAX(period_bars.close) FILTER (
                           WHERE latest_period_bars.is_end_period
                       ) AS end_close,
                       MAX(period_bars.close) FILTER (
                           WHERE NOT latest_period_bars.is_end_period
                       ) AS start_close
                FROM latest_period_bars
                JOIN period_bars
                  ON period_bars.stock_id = latest_period_bars.stock_id
                 AND period_bars.is_end_period = latest_period_bars.is_end_period
                 AND period_bars.timestamp = latest_period_bars.timestamp
                GROUP BY latest_period_bars.stock_id
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
        "#,
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
                       ROUND(
                           SUM(ranked_returns.change_rate * ranked_returns.avg_turnover)
                               / NULLIF(SUM(ranked_returns.avg_turnover), 0),
                           10
                       ) AS turnover_weighted_change_rate,
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
}
