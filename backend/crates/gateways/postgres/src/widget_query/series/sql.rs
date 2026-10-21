use std::sync::LazyLock;

use crate::group_query_sql::with_shared_market_ctes;

const REQUESTED_AXES_SQL: &str = r#"
    SELECT $2::text AS axis_key WHERE $2::text IS NOT NULL
    UNION
    SELECT $3::text AS axis_key WHERE $3::text IS NOT NULL
"#;

const CANDIDATE_STOCKS_CTE: &str = r#"
    candidate_stocks AS MATERIALIZED (
        SELECT stock.id,
               stock.name,
               CASE
                   WHEN stock.id ~ '^[A-Z]{2}:' THEN NULL
                   ELSE LEFT(stock.id, 4)
               END AS valuation_code,
               CASE
                   WHEN stock.id ~ '^[A-Z]{2}:' THEN NULL
                   ELSE 'JPY'
               END AS currency_code
        FROM group_members AS universe_member
        JOIN stock ON stock.id = universe_member.stock_id
        WHERE $3::text IS NOT NULL
          AND universe_member.axis_key = $3::text
          AND universe_member.group_key = $4::text
          AND (cardinality($5::text[]) = 0 OR stock.id = ANY($5::text[]))
          AND (
              $1::text <> 'group'
              OR EXISTS (
                  SELECT 1
                  FROM group_members AS axis_member
                  WHERE axis_member.stock_id = stock.id
                    AND axis_member.axis_key = $2::text
              )
          )
        UNION ALL
        SELECT stock.id,
               stock.name,
               CASE
                   WHEN stock.id ~ '^[A-Z]{2}:' THEN NULL
                   ELSE LEFT(stock.id, 4)
               END AS valuation_code,
               CASE
                   WHEN stock.id ~ '^[A-Z]{2}:' THEN NULL
                   ELSE 'JPY'
               END AS currency_code
        FROM stock
        WHERE $3::text IS NULL
          AND (cardinality($5::text[]) = 0 OR stock.id = ANY($5::text[]))
          AND (
              $1::text <> 'group'
              OR EXISTS (
                  SELECT 1
                  FROM group_members AS axis_member
                  WHERE axis_member.stock_id = stock.id
                    AND axis_member.axis_key = $2::text
              )
          )
    )
"#;

pub(super) static COUNT_CANDIDATES_SQL: LazyLock<String> = LazyLock::new(|| {
    with_shared_market_ctes(
        REQUESTED_AXES_SQL,
        CANDIDATE_STOCKS_CTE,
        "series_marker AS (SELECT 1 AS marker)",
        "candidate_count AS (SELECT count(*)::bigint AS candidate_count FROM candidate_stocks) SELECT candidate_count FROM candidate_count",
    )
});

const STOCK_SERIES_AGGREGATION_SQL: &str = r#"
            stock_values AS (
                SELECT stock_bar_series.stock_id,
                       stock_bar_series.stock_name,
                       stock_bar_series.currency_code,
                       stock_bar_series.date,
                       CASE $9::text
                           WHEN 'change' THEN stock_bar_series.change_rate
                           WHEN 'close' THEN stock_bar_series.close
                           WHEN 'turnover' THEN stock_bar_series.close * stock_bar_series.volume
                           ELSE stock_bar_series.valuation_value
                       END AS value,
                       CASE
                           WHEN $9::text = 'mkt_cap'
                             OR ($9::text = 'change' AND $11::text = 'mkt_cap_weighted')
                           THEN stock_bar_series.valuation_value
                       END AS mkt_cap
                FROM stock_bar_series
                WHERE stock_bar_series.date >= $7::date
            ),
            grouped_values AS (
                SELECT stock_values.date,
                       CASE $1::text
                           WHEN 'market' THEN 'market'
                           WHEN 'stock' THEN stock_values.stock_id
                           WHEN 'group' THEN query_group.group_key
                       END AS row_key,
                       CASE $1::text
                           WHEN 'market' THEN '全体'
                           WHEN 'stock' THEN stock_values.stock_name
                           WHEN 'group' THEN query_group.group_name
                       END AS row_name,
                       stock_values.value,
                       stock_values.mkt_cap,
                       stock_values.currency_code,
                       (
                           stock_values.value IS NOT NULL
                           AND (
                               $11::text IS DISTINCT FROM 'mkt_cap_weighted'
                               OR stock_values.mkt_cap IS NOT NULL
                           )
                       ) AS contributes_to_aggregate
                FROM stock_values
                LEFT JOIN group_members AS query_group
                  ON query_group.stock_id = stock_values.stock_id
                 AND query_group.axis_key = $2::text
                 AND $1::text = 'group'
            ),
            aggregated_values AS (
                SELECT date,
                       row_key,
                       row_name,
                       CASE
                           WHEN $1::text = 'stock' THEN MAX(value)
                           WHEN $11::text = 'mean' THEN AVG(value)
                           WHEN $11::text = 'sum' THEN SUM(value)
                           WHEN $11::text = 'count' THEN COUNT(value)::numeric
                           WHEN $11::text = 'mkt_cap_weighted' THEN
                               SUM(value * mkt_cap)
                                   / NULLIF(SUM(mkt_cap) FILTER (WHERE value IS NOT NULL), 0)
                       END AS value,
                       $12::boolean
                           AND COUNT(*) FILTER (WHERE contributes_to_aggregate) > 1
                           AND COUNT(*) FILTER (
                               WHERE contributes_to_aggregate
                                 AND currency_code IS NULL
                           ) > 0 AS incompatible_currencies
                FROM grouped_values
                GROUP BY date, row_key, row_name
            )
            SELECT date, row_key, row_name, value, incompatible_currencies
            FROM aggregated_values
            WHERE row_key IS NOT NULL
            ORDER BY date, row_key
        "#;

pub(super) static STOCK_SERIES_SQL: LazyLock<String> = LazyLock::new(|| {
    with_shared_market_ctes(
        REQUESTED_AXES_SQL,
        CANDIDATE_STOCKS_CTE,
        r#"
            stock_bar_series AS (
                SELECT candidate_stocks.id AS stock_id,
                       candidate_stocks.name AS stock_name,
                       candidate_stocks.currency_code,
                       stock_series.date,
                       stock_series.close,
                       stock_series.volume,
                       stock_series.change_rate,
                       stock_series.valuation_value
                FROM candidate_stocks
                JOIN LATERAL (
                    WITH valuation_before_range AS (
                        SELECT valuation.date,
                               valuation.code,
                               valuation.mkt_cap,
                               valuation.per,
                               valuation.pbr
                        FROM valuation
                        WHERE LEFT(valuation.code, 4) = candidate_stocks.valuation_code
                          AND valuation.date < $7::date
                          AND (
                              (($9::text = 'mkt_cap'
                                  OR ($9::text = 'change' AND $11::text = 'mkt_cap_weighted'))
                                  AND valuation.mkt_cap IS NOT NULL)
                              OR ($9::text = 'per' AND valuation.per IS NOT NULL)
                              OR ($9::text = 'pbr' AND valuation.pbr IS NOT NULL)
                          )
                        ORDER BY valuation.date DESC, valuation.code DESC
                        LIMIT 1
                    ),
                    valuation_in_range AS (
                        SELECT DISTINCT ON (valuation.date)
                               valuation.date,
                               valuation.code,
                               valuation.mkt_cap,
                               valuation.per,
                               valuation.pbr
                        FROM valuation
                        WHERE LEFT(valuation.code, 4) = candidate_stocks.valuation_code
                          AND valuation.date >= $7::date
                          AND valuation.date <= $8::date
                          AND (
                              (($9::text = 'mkt_cap'
                                  OR ($9::text = 'change' AND $11::text = 'mkt_cap_weighted'))
                                  AND valuation.mkt_cap IS NOT NULL)
                              OR ($9::text = 'per' AND valuation.per IS NOT NULL)
                              OR ($9::text = 'pbr' AND valuation.pbr IS NOT NULL)
                          )
                        ORDER BY valuation.date DESC, valuation.code DESC
                    ),
                    valuation_records AS MATERIALIZED (
                        SELECT valuation_in_range.date,
                               CASE
                                   WHEN $9::text = 'mkt_cap'
                                     OR ($9::text = 'change' AND $11::text = 'mkt_cap_weighted')
                                   THEN valuation_in_range.mkt_cap
                                   WHEN $9::text = 'per' THEN valuation_in_range.per
                                   WHEN $9::text = 'pbr' THEN valuation_in_range.pbr
                               END AS valuation_value
                        FROM valuation_in_range
                        UNION ALL
                        SELECT valuation_before_range.date,
                               CASE
                                   WHEN $9::text = 'mkt_cap'
                                     OR ($9::text = 'change' AND $11::text = 'mkt_cap_weighted')
                                   THEN valuation_before_range.mkt_cap
                                   WHEN $9::text = 'per' THEN valuation_before_range.per
                                   WHEN $9::text = 'pbr' THEN valuation_before_range.pbr
                               END AS valuation_value
                        FROM valuation_before_range
                    ),
                    bar_events AS MATERIALIZED (
                        SELECT (daily_bars.timestamp AT TIME ZONE 'UTC')::date AS date,
                               daily_bars.close,
                               daily_bars.volume,
                               CASE
                                   WHEN $9::text = 'change' THEN daily_bars.close / NULLIF(
                                       LAG(daily_bars.close, $10::integer) OVER (
                                           ORDER BY daily_bars.timestamp
                                       ),
                                       0
                                   ) - 1
                               END AS change_rate
                        FROM (
                            SELECT warmup_bars.timestamp,
                                   warmup_bars.close,
                                   warmup_bars.volume
                            FROM (
                                SELECT daily_bar.timestamp,
                                       daily_bar.close,
                                       daily_bar.volume
                                FROM bars AS daily_bar
                                WHERE daily_bar.instrument_id = candidate_stocks.id
                                  AND daily_bar.timeframe = '1d'
                                  AND daily_bar.timestamp >= (
                                      ($6::date::timestamp) AT TIME ZONE 'UTC'
                                  )
                                  AND daily_bar.timestamp < (
                                      ($7::date::timestamp) AT TIME ZONE 'UTC'
                                  )
                                ORDER BY daily_bar.timestamp DESC
                                LIMIT CASE
                                    WHEN $9::text = 'change' THEN $10::integer
                                    ELSE 0
                                END
                            ) AS warmup_bars
                            UNION ALL
                            SELECT daily_bar.timestamp,
                                   daily_bar.close,
                                   daily_bar.volume
                            FROM bars AS daily_bar
                            WHERE daily_bar.instrument_id = candidate_stocks.id
                              AND daily_bar.timeframe = '1d'
                              AND daily_bar.timestamp >= (
                                  ($7::date::timestamp) AT TIME ZONE 'UTC'
                              )
                              AND daily_bar.timestamp < (
                                  (($8::date + 1)::timestamp) AT TIME ZONE 'UTC'
                              )
                        ) AS daily_bars
                    ),
                    valuation_coverage AS (
                        SELECT COUNT(*) = COUNT(valuation_records.valuation_value) AS is_complete
                        FROM bar_events
                        LEFT JOIN valuation_records
                          ON valuation_records.date = bar_events.date
                        WHERE bar_events.date >= $7::date
                    ),
                    dense_series AS (
                        SELECT bar_events.date,
                               bar_events.close,
                               bar_events.volume,
                               bar_events.change_rate,
                               valuation_records.valuation_value
                        FROM bar_events
                        JOIN valuation_records
                          ON valuation_records.date = bar_events.date
                        CROSS JOIN valuation_coverage
                        WHERE bar_events.date >= $7::date
                          AND valuation_coverage.is_complete
                    ),
                    series_events AS (
                        SELECT bar_events.date,
                               bar_events.close,
                               bar_events.volume,
                               bar_events.change_rate,
                               NULL::numeric AS valuation_value
                        FROM bar_events
                        CROSS JOIN valuation_coverage
                        WHERE bar_events.date >= $7::date
                          AND NOT valuation_coverage.is_complete
                        UNION ALL
                        SELECT valuation_records.date,
                               NULL::numeric AS close,
                               NULL::bigint AS volume,
                               NULL::numeric AS change_rate,
                               valuation_records.valuation_value
                        FROM valuation_records
                        CROSS JOIN valuation_coverage
                        WHERE NOT valuation_coverage.is_complete
                    ),
                    series_events_by_date AS (
                        SELECT date,
                               MAX(close) AS close,
                               MAX(volume) AS volume,
                               MAX(change_rate) AS change_rate,
                               MAX(valuation_value) AS valuation_value
                        FROM series_events
                        GROUP BY date
                    ),
                    series_events_with_groups AS (
                        SELECT series_events_by_date.*,
                               COUNT(valuation_value) OVER (
                                   ORDER BY date ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
                               ) AS valuation_group
                        FROM series_events_by_date
                    ),
                    series_events_with_values AS (
                        SELECT series_events_with_groups.*,
                               MAX(valuation_value) OVER (PARTITION BY valuation_group) AS filled_valuation_value
                        FROM series_events_with_groups
                    ),
                    sparse_series AS (
                        SELECT date,
                               close,
                               volume,
                               change_rate,
                               filled_valuation_value AS valuation_value
                        FROM series_events_with_values
                        WHERE close IS NOT NULL
                          AND date >= $7::date
                    ),
                    series_result AS (
                        SELECT * FROM dense_series
                        UNION ALL
                        SELECT * FROM sparse_series
                    )
                    SELECT date, close, volume, change_rate, valuation_value
                    FROM series_result
                ) AS stock_series ON TRUE
            )
        "#,
        STOCK_SERIES_AGGREGATION_SQL,
    )
});

pub(super) const INDICATOR_SERIES_SQL: &str = r#"
    SELECT 'market'::text AS row_key,
           indicator.name AS row_name,
           observation.date,
           observation.value,
           false AS incompatible_currencies
    FROM indicator
    JOIN indicator_observation AS observation
      ON observation.indicator_id = indicator.id
    WHERE indicator.id = $1::text
      AND observation.date >= $2::date
      AND observation.date <= $3::date
    ORDER BY observation.date
"#;
