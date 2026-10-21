use std::sync::LazyLock;

use chrono::NaiveDate;
use core_application::widget::{
    MAX_WIDGET_QUERY_POINTS, ResolvedWidgetTimeRange, WidgetAggregation, WidgetBy, WidgetMetric,
    WidgetQuery, WidgetQueryError, WidgetRowKey, WidgetSeriesQueryRow,
};
use sea_orm::{DatabaseBackend, FromQueryResult, Statement};

use super::{PostgresWidgetQuerySource, query_error, timeout::query_all_with_timeout};
use crate::group_query_sql::with_shared_market_ctes;

const REQUESTED_AXES_SQL: &str = r#"
    SELECT $2::text AS axis_key WHERE $2::text IS NOT NULL
    UNION
    SELECT $3::text AS axis_key WHERE $3::text IS NOT NULL
"#;

const CANDIDATE_STOCKS_CTE: &str = r#"
    candidate_stocks AS (
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
        WHERE (cardinality($5::text[]) = 0 OR stock.id = ANY($5::text[]))
          AND (
              $3::text IS NULL
              OR EXISTS (
                  SELECT 1
                  FROM group_members AS universe_member
                  WHERE universe_member.stock_id = stock.id
                    AND universe_member.axis_key = $3::text
                    AND universe_member.group_key = $4::text
              )
          )
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

static COUNT_CANDIDATES_SQL: LazyLock<String> = LazyLock::new(|| {
    with_shared_market_ctes(
        REQUESTED_AXES_SQL,
        CANDIDATE_STOCKS_CTE,
        "series_marker AS (SELECT 1 AS marker)",
        "candidate_count AS (SELECT count(*)::bigint AS candidate_count FROM candidate_stocks) SELECT candidate_count FROM candidate_count",
    )
});

static STOCK_SERIES_SQL: LazyLock<String> = LazyLock::new(|| {
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
                       stock_series.mkt_cap,
                       stock_series.per,
                       stock_series.pbr
                FROM candidate_stocks
                JOIN LATERAL (
                    WITH valuation_scoped AS MATERIALIZED (
                        SELECT candidate_stocks.id AS stock_id,
                               valuation.date,
                               valuation.code,
                               valuation.mkt_cap,
                               valuation.per,
                               valuation.pbr
                        FROM valuation
                        WHERE LEFT(valuation.code, 4) = candidate_stocks.valuation_code
                          AND valuation.date <= $8::date
                          AND (
                              (($9::text = 'mkt_cap'
                                  OR ($9::text = 'change' AND $11::text = 'mkt_cap_weighted'))
                                  AND valuation.mkt_cap IS NOT NULL)
                              OR ($9::text = 'per' AND valuation.per IS NOT NULL)
                              OR ($9::text = 'pbr' AND valuation.pbr IS NOT NULL)
                          )
                    ),
                    valuation_before_range AS (
                        SELECT DISTINCT ON (stock_id)
                               stock_id, date, code, mkt_cap, per, pbr
                        FROM valuation_scoped
                        WHERE date < $7::date
                        ORDER BY stock_id, date DESC, code DESC
                    ),
                    valuation_in_range AS (
                        SELECT DISTINCT ON (stock_id, date)
                               stock_id, date, code, mkt_cap, per, pbr
                        FROM valuation_scoped
                        WHERE date >= $7::date
                          AND date <= $8::date
                        ORDER BY stock_id, date, code DESC
                    ),
                    valuation_records AS (
                        SELECT * FROM valuation_before_range
                        UNION ALL
                        SELECT * FROM valuation_in_range
                    ),
                    valuation_intervals AS (
                        SELECT stock_id,
                               date,
                               LEAD(date, 1, $8::date + 1) OVER (
                                   PARTITION BY stock_id ORDER BY date
                               ) AS next_date,
                               mkt_cap,
                               per,
                               pbr
                        FROM valuation_records
                    )
                    SELECT stock_bars.date,
                           stock_bars.close,
                           stock_bars.volume,
                           stock_bars.change_rate,
                           valuation_intervals.mkt_cap,
                           valuation_intervals.per,
                           valuation_intervals.pbr
                    FROM (
                        SELECT daily_bars.timestamp::date AS date,
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
                    ) AS stock_bars
                    LEFT JOIN valuation_intervals
                      ON valuation_intervals.stock_id = candidate_stocks.id
                     AND stock_bars.date >= valuation_intervals.date
                     AND stock_bars.date < valuation_intervals.next_date
                    WHERE stock_bars.date >= $7::date
                ) AS stock_series ON TRUE
            )
        "#,
        r#"
            stock_values AS (
                SELECT stock_bar_series.stock_id,
                       stock_bar_series.stock_name,
                       stock_bar_series.currency_code,
                       stock_bar_series.date,
                       CASE $9::text
                           WHEN 'change' THEN stock_bar_series.change_rate
                           WHEN 'close' THEN stock_bar_series.close
                           WHEN 'turnover' THEN stock_bar_series.close * stock_bar_series.volume
                           WHEN 'mkt_cap' THEN stock_bar_series.mkt_cap
                           WHEN 'per' THEN stock_bar_series.per
                           WHEN 'pbr' THEN stock_bar_series.pbr
                       END AS value,
                       stock_bar_series.mkt_cap
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
        "#,
    )
});

const INDICATOR_SERIES_SQL: &str = r#"
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

#[derive(Debug, FromQueryResult)]
struct CandidateCountRow {
    candidate_count: i64,
}

#[derive(Debug, FromQueryResult)]
struct WidgetSeriesResultRow {
    date: NaiveDate,
    row_key: String,
    row_name: String,
    value: Option<rust_decimal::Decimal>,
    incompatible_currencies: bool,
}

pub(super) async fn query_series(
    source: &PostgresWidgetQuerySource,
    range: ResolvedWidgetTimeRange,
    definition: &WidgetQuery,
) -> Result<Vec<WidgetSeriesQueryRow>, WidgetQueryError> {
    if definition.metric == WidgetMetric::Indicator {
        return query_indicator_series(source, range, definition).await;
    }

    let (universe_axis, universe_key) = definition
        .universe
        .as_ref()
        .and_then(|universe| universe.group_parts())
        .map(|(axis, key)| (Some(axis.to_string()), Some(key.to_string())))
        .unwrap_or((None, None));
    let stock_ids = definition
        .universe
        .as_ref()
        .and_then(|universe| universe.stock_ids())
        .map(<[String]>::to_vec)
        .unwrap_or_default();
    let axis = definition.axis.clone();
    let by = query_by_sql(definition.by);
    let candidate_rows = query_all_with_timeout(
        &source.db,
        Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            COUNT_CANDIDATES_SQL.as_str(),
            [
                by.into(),
                axis.clone().into(),
                universe_axis.clone().into(),
                universe_key.clone().into(),
                stock_ids.clone().into(),
            ],
        ),
    )
    .await?;
    let candidate_count = candidate_rows
        .first()
        .map(|row| CandidateCountRow::from_query_result(row, "").map_err(query_error))
        .transpose()?
        .map(|row| u64::try_from(row.candidate_count).unwrap_or(u64::MAX))
        .unwrap_or_default();
    let input_points = candidate_count.saturating_mul(range.input_calendar_days());
    if input_points > MAX_WIDGET_QUERY_POINTS {
        return Err(WidgetQueryError::InputLimitExceeded {
            limit: MAX_WIDGET_QUERY_POINTS,
        });
    }

    let aggregation = definition.agg.map(WidgetAggregation::as_str);
    let period = definition
        .period
        .map(|period| period.trading_days())
        .unwrap_or(1);
    let requires_currency_consistency = match definition.agg {
        Some(WidgetAggregation::MarketCapWeighted) => true,
        Some(WidgetAggregation::Mean | WidgetAggregation::Sum) => matches!(
            definition.metric,
            WidgetMetric::Close | WidgetMetric::Turnover | WidgetMetric::MktCap
        ),
        Some(WidgetAggregation::Count) | None => false,
    } && definition.by != WidgetBy::Stock;
    let rows = query_all_with_timeout(
        &source.db,
        Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            STOCK_SERIES_SQL.as_str(),
            [
                by.into(),
                axis.into(),
                universe_axis.into(),
                universe_key.into(),
                stock_ids.into(),
                range.query_from.into(),
                range.from.into(),
                range.to.into(),
                definition.metric.as_str().into(),
                period.into(),
                aggregation.map(str::to_string).into(),
                requires_currency_consistency.into(),
            ],
        ),
    )
    .await?;
    let rows = rows
        .iter()
        .map(|row| WidgetSeriesResultRow::from_query_result(row, "").map_err(query_error))
        .collect::<Result<Vec<_>, _>>()?;
    if rows.iter().any(|row| row.incompatible_currencies) {
        return Err(WidgetQueryError::IncompatibleCurrencies);
    }
    rows.into_iter()
        .map(|row| {
            let key = match definition.by {
                WidgetBy::None => WidgetRowKey::Market,
                WidgetBy::Stock => WidgetRowKey::Stock(row.row_key),
                WidgetBy::Group => WidgetRowKey::Group {
                    axis: definition.axis.clone().unwrap_or_default(),
                    key: row.row_key,
                },
            };
            Ok(WidgetSeriesQueryRow {
                key,
                name: row.row_name,
                date: row.date,
                value: row.value,
            })
        })
        .collect()
}

async fn query_indicator_series(
    source: &PostgresWidgetQuerySource,
    range: ResolvedWidgetTimeRange,
    definition: &WidgetQuery,
) -> Result<Vec<WidgetSeriesQueryRow>, WidgetQueryError> {
    let rows = query_all_with_timeout(
        &source.db,
        Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            INDICATOR_SERIES_SQL,
            [
                definition.indicator.clone().unwrap_or_default().into(),
                range.from.into(),
                range.to.into(),
            ],
        ),
    )
    .await?;
    rows.iter()
        .map(|row| {
            let row = WidgetSeriesResultRow::from_query_result(row, "").map_err(query_error)?;
            Ok(WidgetSeriesQueryRow {
                key: WidgetRowKey::Market,
                name: row.row_name,
                date: row.date,
                value: row.value,
            })
        })
        .collect()
}

fn query_by_sql(by: WidgetBy) -> &'static str {
    match by {
        WidgetBy::None => "market",
        WidgetBy::Stock | WidgetBy::Group => by.as_str(),
    }
}
