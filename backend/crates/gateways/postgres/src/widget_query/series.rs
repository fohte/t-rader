mod sql;

use chrono::NaiveDate;
use core_application::widget::{
    MAX_WIDGET_QUERY_POINTS, ResolvedWidgetTimeRange, WidgetAggregation, WidgetBy, WidgetMetric,
    WidgetQuery, WidgetQueryError, WidgetRowKey, WidgetSeriesQueryRow,
};
use sea_orm::{DatabaseBackend, FromQueryResult, Statement};

use super::{PostgresWidgetQuerySource, query_error, timeout::query_all_with_timeout};
use sql::{COUNT_CANDIDATES_SQL, INDICATOR_SERIES_SQL, STOCK_SERIES_SQL};

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
    let statement = Statement::from_sql_and_values(
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
    );
    let rows = query_all_with_timeout(&source.db, statement).await?;
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
