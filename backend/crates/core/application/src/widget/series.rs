use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::{
    formula::FormulaExpression,
    query::{WidgetQuerySource, WidgetRowKey, WidgetUseCaseError},
    range::WidgetTimeRange,
    types::{WidgetBy, WidgetDefinition, WidgetQuery, WidgetVisualization},
    validation::{WidgetValidationError, WidgetValidationErrors},
};

pub const MAX_WIDGET_LINE_SERIES: usize = 50;
pub const MAX_WIDGET_QUERY_POINTS: u64 = 250_000;

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetSeries {
    pub key: String,
    pub query: String,
    pub name: String,
    pub target: Option<WidgetSeriesTarget>,
    pub points: Vec<WidgetSeriesPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetSeriesTarget {
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetSeriesPoint {
    pub date: NaiveDate,
    pub value: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetSeriesQueryRow {
    pub key: WidgetRowKey,
    pub name: String,
    pub date: NaiveDate,
    pub value: Option<Decimal>,
}

pub async fn query_line(
    source: &dyn WidgetQuerySource,
    definition: &WidgetDefinition,
    parsed_formulas: &BTreeMap<&str, FormulaExpression>,
    time_range: WidgetTimeRange,
    as_of: NaiveDate,
) -> Result<Vec<WidgetSeries>, WidgetUseCaseError> {
    let mut query_values = BTreeMap::new();
    for (name, query) in &definition.queries {
        let range = time_range.resolve(as_of, query.period).map_err(|message| {
            WidgetUseCaseError::Validation(WidgetValidationErrors(vec![WidgetValidationError {
                path: "time.range".to_string(),
                message,
            }]))
        })?;
        let values = source.query_series(range, query).await?;
        let mut by_date = BTreeMap::new();
        for value in values {
            by_date
                .entry(value.date)
                .or_insert_with(BTreeMap::new)
                .insert(value.key, (value.name, value.value));
        }
        query_values.insert(name.as_str(), by_date);
    }

    let WidgetVisualization::Line { series } = &definition.viz else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    for name in series {
        let expression = parsed_formulas.get(name.as_str());
        let query = definition.queries.get(name);
        let references = expression
            .map(FormulaExpression::references)
            .unwrap_or_else(|| BTreeSet::from([name.as_str()]));
        let row_keys = series_row_keys(name, expression, query, definition, &query_values);
        for key in row_keys {
            let dates = series_dates(name, expression, query, &key, &query_values);
            let points = dates
                .into_iter()
                .map(|date| WidgetSeriesPoint {
                    date,
                    value: match (query, expression) {
                        (Some(query), _) => {
                            query_value(query, &key, date, query_values.get(name.as_str()))
                        }
                        (_, Some(expression)) => expression.evaluate(&|reference| {
                            let query = definition.queries.get(reference)?;
                            query_value(query, &key, date, query_values.get(reference))
                        }),
                        (None, None) => None,
                    },
                })
                .collect();
            let label = definition
                .display
                .get(name)
                .and_then(|display| display.label.as_ref())
                .cloned()
                .unwrap_or_else(|| name.clone());
            let (key, target) = match key {
                WidgetRowKey::Market => (name.clone(), None),
                key => {
                    let target_name = row_name(&key, &references, &query_values);
                    (
                        format!("{name}:{}", key.as_string()),
                        Some(WidgetSeriesTarget {
                            key: key.as_string(),
                            name: target_name,
                        }),
                    )
                }
            };
            result.push(WidgetSeries {
                key,
                query: name.clone(),
                name: label,
                target,
                points,
            });
        }
    }

    if result.len() > MAX_WIDGET_LINE_SERIES {
        return Err(WidgetUseCaseError::Validation(WidgetValidationErrors(
            vec![WidgetValidationError {
                path: "viz.series".to_string(),
                message: format!(
                    "line visualization must not produce more than {MAX_WIDGET_LINE_SERIES} series"
                ),
            }],
        )));
    }
    Ok(result)
}

type SeriesValues = BTreeMap<NaiveDate, BTreeMap<WidgetRowKey, (String, Option<Decimal>)>>;

fn series_row_keys(
    name: &str,
    expression: Option<&FormulaExpression>,
    query: Option<&WidgetQuery>,
    definition: &WidgetDefinition,
    query_values: &BTreeMap<&str, SeriesValues>,
) -> BTreeSet<WidgetRowKey> {
    if let Some(query) = query {
        if query.by == WidgetBy::None {
            return BTreeSet::from([WidgetRowKey::Market]);
        }
        return query_values
            .get(name)
            .into_iter()
            .flat_map(|dates| dates.values())
            .flat_map(|values| values.keys().cloned())
            .collect();
    }

    let row_references = expression
        .map(FormulaExpression::references)
        .unwrap_or_default()
        .into_iter()
        .filter(|reference| {
            definition
                .queries
                .get(*reference)
                .is_some_and(|query| query.by != WidgetBy::None)
        })
        .collect::<BTreeSet<_>>();
    if row_references.is_empty() {
        return BTreeSet::from([WidgetRowKey::Market]);
    }
    row_references
        .iter()
        .filter_map(|reference| query_values.get(reference))
        .flat_map(|dates| dates.values())
        .flat_map(|values| values.keys().cloned())
        .collect()
}

fn series_dates(
    name: &str,
    expression: Option<&FormulaExpression>,
    query: Option<&WidgetQuery>,
    key: &WidgetRowKey,
    query_values: &BTreeMap<&str, SeriesValues>,
) -> BTreeSet<NaiveDate> {
    if query.is_some() {
        return query_values
            .get(name)
            .into_iter()
            .flat_map(|dates| dates.iter())
            .filter_map(|(date, values)| values.contains_key(key).then_some(*date))
            .collect();
    }

    let Some(expression) = expression else {
        return BTreeSet::new();
    };
    expression
        .references()
        .into_iter()
        .filter_map(|reference| query_values.get(reference))
        .flat_map(|dates| dates.iter())
        .filter_map(|(date, values)| {
            (values.contains_key(key) || values.contains_key(&WidgetRowKey::Market))
                .then_some(*date)
        })
        .collect()
}

fn query_value(
    query: &WidgetQuery,
    key: &WidgetRowKey,
    date: NaiveDate,
    values: Option<&SeriesValues>,
) -> Option<Decimal> {
    let market_key = WidgetRowKey::Market;
    let key = if query.by == WidgetBy::None {
        &market_key
    } else {
        key
    };
    values?.get(&date)?.get(key).and_then(|(_, value)| *value)
}

fn row_name(
    key: &WidgetRowKey,
    references: &BTreeSet<&str>,
    query_values: &BTreeMap<&str, SeriesValues>,
) -> String {
    references
        .iter()
        .filter_map(|reference| query_values.get(reference))
        .flat_map(|dates| dates.values())
        .find_map(|values| values.get(key).map(|(name, _)| name.clone()))
        .unwrap_or_else(|| key.as_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use async_trait::async_trait;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;

    use super::{
        WidgetSeries, WidgetSeriesPoint, WidgetSeriesQueryRow, WidgetSeriesTarget, query_line,
    };
    use crate::widget::query::{WidgetQueryError, WidgetQueryRow, WidgetQuerySource, WidgetRowKey};
    use crate::widget::{
        MAX_WIDGET_LINE_SERIES, WidgetBy, WidgetDefinition, WidgetMetric, WidgetPeriod,
        WidgetQuery, WidgetTime, WidgetTimeRange, WidgetVisualization,
    };

    struct FakeSeriesSource {
        rows: Vec<WidgetSeriesQueryRow>,
    }

    #[async_trait]
    impl WidgetQuerySource for FakeSeriesSource {
        async fn resolve_as_of(
            &self,
            requested: Option<NaiveDate>,
        ) -> Result<Option<NaiveDate>, WidgetQueryError> {
            Ok(requested.or(Some(date(2098, 1, 5))))
        }

        async fn query(
            &self,
            _as_of: NaiveDate,
            _definition: &WidgetQuery,
        ) -> Result<Vec<WidgetQueryRow>, WidgetQueryError> {
            Ok(Vec::new())
        }

        async fn query_series(
            &self,
            range: crate::widget::ResolvedWidgetTimeRange,
            definition: &WidgetQuery,
        ) -> Result<Vec<WidgetSeriesQueryRow>, WidgetQueryError> {
            Ok(self
                .rows
                .iter()
                .filter(|row| {
                    row.date >= range.from
                        && row.date <= range.to
                        && (definition.metric == WidgetMetric::Indicator
                            && row.key == WidgetRowKey::Market
                            || definition.metric != WidgetMetric::Indicator
                                && row.key != WidgetRowKey::Market)
                })
                .cloned()
                .collect())
        }
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn query(by: WidgetBy, metric: WidgetMetric) -> WidgetQuery {
        WidgetQuery {
            metric,
            period: (metric == WidgetMetric::Change).then_some(WidgetPeriod::OneDay),
            by,
            axis: None,
            indicator: None,
            universe: None,
            agg: None,
        }
    }

    fn definition() -> WidgetDefinition {
        WidgetDefinition {
            queries: BTreeMap::from([
                (
                    "daily".to_string(),
                    query(WidgetBy::Stock, WidgetMetric::Close),
                ),
                (
                    "market".to_string(),
                    query(WidgetBy::None, WidgetMetric::Indicator),
                ),
            ]),
            formulas: BTreeMap::from([("combined".to_string(), "daily + market".to_string())]),
            display: BTreeMap::new(),
            viz: WidgetVisualization::Line {
                series: vec!["combined".to_string()],
            },
            time: WidgetTime {
                as_of: Some(date(2098, 1, 6)),
                range: Some("2098-01-05..2098-01-06".to_string()),
            },
        }
    }

    #[tokio::test]
    async fn line_formulas_match_rows_and_broadcast_market_values_on_the_same_date() {
        let source = FakeSeriesSource {
            rows: vec![
                WidgetSeriesQueryRow {
                    key: WidgetRowKey::Stock("DEMO-A".to_string()),
                    name: "Demo Stock A".to_string(),
                    date: date(2098, 1, 5),
                    value: Some(Decimal::new(100, 0)),
                },
                WidgetSeriesQueryRow {
                    key: WidgetRowKey::Market,
                    name: "Demo Indicator".to_string(),
                    date: date(2098, 1, 5),
                    value: Some(Decimal::new(2, 0)),
                },
                WidgetSeriesQueryRow {
                    key: WidgetRowKey::Market,
                    name: "Demo Indicator".to_string(),
                    date: date(2098, 1, 6),
                    value: Some(Decimal::new(3, 0)),
                },
            ],
        };
        let definition = definition();
        let parsed_formulas = BTreeMap::from([(
            "combined",
            crate::widget::formula::parse_formula("daily + market").expect("valid formula"),
        )]);

        let result = query_line(
            &source,
            &definition,
            &parsed_formulas,
            WidgetTimeRange::parse("2098-01-05..2098-01-06").expect("valid range"),
            date(2098, 1, 6),
        )
        .await
        .expect("calculate line series");

        assert_eq!(
            result,
            vec![WidgetSeries {
                key: "combined:stock:DEMO-A".to_string(),
                query: "combined".to_string(),
                name: "combined".to_string(),
                target: Some(WidgetSeriesTarget {
                    key: "stock:DEMO-A".to_string(),
                    name: "Demo Stock A".to_string(),
                }),
                points: vec![
                    WidgetSeriesPoint {
                        date: date(2098, 1, 5),
                        value: Some(Decimal::new(102, 0)),
                    },
                    WidgetSeriesPoint {
                        date: date(2098, 1, 6),
                        value: None,
                    },
                ],
            }]
        );
    }

    #[tokio::test]
    async fn rejects_more_than_fifty_line_series() {
        let source = FakeSeriesSource {
            rows: (0..=MAX_WIDGET_LINE_SERIES)
                .map(|index| WidgetSeriesQueryRow {
                    key: WidgetRowKey::Stock(format!("DEMO-{index}")),
                    name: format!("Demo Stock {index}"),
                    date: date(2098, 1, 5),
                    value: Some(Decimal::from(index)),
                })
                .collect(),
        };
        let mut definition = definition();
        definition.queries = BTreeMap::from([(
            "daily".to_string(),
            query(WidgetBy::Stock, WidgetMetric::Close),
        )]);
        definition.formulas.clear();
        definition.viz = WidgetVisualization::Line {
            series: vec!["daily".to_string()],
        };
        let result = query_line(
            &source,
            &definition,
            &BTreeMap::new(),
            WidgetTimeRange::parse("2098-01-05..2098-01-05").expect("valid range"),
            date(2098, 1, 5),
        )
        .await;

        assert_eq!(
            result.map_err(|error| error.to_string()),
            Err("viz.series: line visualization must not produce more than 50 series".to_string())
        );
    }
}
