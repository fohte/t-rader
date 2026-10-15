use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use thiserror::Error;

use crate::persistence::PersistenceError;

use super::{
    formula::parse_formula,
    types::{
        WidgetBy, WidgetDefinition, WidgetQuery, WidgetVisualization, WidgetVisualizationOrder,
    },
    validation::WidgetValidationErrors,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum WidgetRowKey {
    Market,
    Stock(String),
    Group { axis: String, key: String },
}

impl WidgetRowKey {
    pub fn as_string(&self) -> String {
        match self {
            Self::Market => "market".to_string(),
            Self::Stock(id) => format!("stock:{id}"),
            Self::Group { axis, key } => format!("group:{axis}/{key}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetQueryRow {
    pub key: WidgetRowKey,
    pub name: String,
    pub value: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetValueRow {
    pub key: WidgetRowKey,
    pub name: String,
    pub values: BTreeMap<String, Option<Decimal>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetQueryResponse {
    pub as_of: NaiveDate,
    pub rows: Vec<WidgetValueRow>,
}

#[derive(Debug, Error)]
pub enum WidgetQueryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait WidgetQuerySource: Send + Sync {
    async fn resolve_as_of(
        &self,
        requested: Option<NaiveDate>,
    ) -> Result<Option<NaiveDate>, WidgetQueryError>;

    async fn query(
        &self,
        as_of: NaiveDate,
        definition: &WidgetQuery,
    ) -> Result<Vec<WidgetQueryRow>, WidgetQueryError>;
}

pub type SharedWidgetQuerySource = Arc<dyn WidgetQuerySource + Send + Sync>;

#[derive(Debug, Error)]
pub enum WidgetUseCaseError {
    #[error("{0}")]
    Validation(#[from] WidgetValidationErrors),
    #[error("no daily bar data is available on or before the requested date")]
    NoData,
    #[error(transparent)]
    Query(#[from] WidgetQueryError),
    #[error("validated formula could not be parsed: {0}")]
    InvalidFormula(String),
}

#[derive(Clone)]
pub struct WidgetUseCases {
    query_source: SharedWidgetQuerySource,
}

impl WidgetUseCases {
    pub fn new(query_source: SharedWidgetQuerySource) -> Self {
        Self { query_source }
    }

    pub async fn query(
        &self,
        definition: WidgetDefinition,
    ) -> Result<WidgetQueryResponse, WidgetUseCaseError> {
        definition.validate()?;
        let as_of = self
            .query_source
            .resolve_as_of(definition.time.as_of)
            .await?
            .ok_or(WidgetUseCaseError::NoData)?;

        let mut query_values = BTreeMap::new();
        let mut result_rows = BTreeMap::new();

        for (name, query) in &definition.queries {
            let rows = self.query_source.query(as_of, query).await?;
            let mut values = BTreeMap::new();
            for row in rows {
                result_rows
                    .entry(row.key.clone())
                    .or_insert_with(|| WidgetValueRow {
                        key: row.key.clone(),
                        name: row.name.clone(),
                        values: BTreeMap::new(),
                    })
                    .values
                    .insert(name.clone(), Some(row.value));
                values.insert(row.key, (row.name, row.value));
            }
            query_values.insert(name.as_str(), values);
        }

        for (name, query) in &definition.queries {
            if query.by != WidgetBy::None {
                continue;
            }
            let Some(value) = query_values
                .get(name.as_str())
                .and_then(|values| values.get(&WidgetRowKey::Market))
                .map(|(_, value)| *value)
            else {
                continue;
            };
            for (key, row) in &mut result_rows {
                if key != &WidgetRowKey::Market {
                    row.values.insert(name.clone(), Some(value));
                }
            }
        }

        for (name, formula) in &definition.formulas {
            let expression = parse_formula(formula).map_err(WidgetUseCaseError::InvalidFormula)?;
            let references = expression.references();
            let row_source = references.iter().find(|reference| {
                definition
                    .queries
                    .get::<str>(*reference)
                    .is_some_and(|query| query.by != WidgetBy::None)
            });
            let row_keys = match row_source {
                Some(reference) => query_values
                    .get(reference)
                    .map(|values| values.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default(),
                None if query_values
                    .values()
                    .any(|values| values.contains_key(&WidgetRowKey::Market)) =>
                {
                    vec![WidgetRowKey::Market]
                }
                None => Vec::new(),
            };

            for key in row_keys {
                let referenced_values = references
                    .iter()
                    .map(|reference| {
                        let query = definition.queries.get(*reference)?;
                        let query_key = match query.by {
                            WidgetBy::None => WidgetRowKey::Market,
                            WidgetBy::Stock | WidgetBy::Group => key.clone(),
                        };
                        query_values
                            .get(reference)
                            .and_then(|values| values.get(&query_key))
                            .map(|(_, value)| (*reference, *value))
                    })
                    .collect::<Option<BTreeMap<_, _>>>();
                let Some(referenced_values) = referenced_values else {
                    continue;
                };

                let Some(result_row) = result_rows.get_mut(&key) else {
                    continue;
                };

                let value =
                    expression.evaluate(&|reference| referenced_values.get(reference).copied());
                result_row.values.insert(name.clone(), value);
            }
        }

        let rows = project_visualization_rows(result_rows.into_values().collect(), &definition.viz);

        Ok(WidgetQueryResponse { as_of, rows })
    }
}

fn project_visualization_rows(
    mut rows: Vec<WidgetValueRow>,
    visualization: &WidgetVisualization,
) -> Vec<WidgetValueRow> {
    match visualization {
        WidgetVisualization::Treemap { size, color } => rows
            .into_iter()
            .filter(|row| {
                row.values.get(size).is_some_and(Option::is_some)
                    && row.values.get(color).is_some_and(Option::is_some)
            })
            .collect(),
        WidgetVisualization::Toplist {
            value,
            order,
            limit,
        } => {
            rows.retain(|row| row.values.get(value).is_some_and(Option::is_some));
            rows.sort_by(|left, right| {
                let left_value = left.values.get(value).and_then(|value| *value);
                let right_value = right.values.get(value).and_then(|value| *value);
                let order = match (left_value, right_value) {
                    (Some(left), Some(right)) => match order {
                        WidgetVisualizationOrder::Asc => left.cmp(&right),
                        WidgetVisualizationOrder::Desc => right.cmp(&left),
                    },
                    _ => std::cmp::Ordering::Equal,
                };
                order.then_with(|| left.key.cmp(&right.key))
            });
            rows.truncate(*limit as usize);
            rows
        }
        WidgetVisualization::Table { .. } | WidgetVisualization::Line { .. } => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use async_trait::async_trait;
    use chrono::NaiveDate;
    use rstest::rstest;
    use rust_decimal::Decimal;

    use super::{
        SharedWidgetQuerySource, WidgetQueryError, WidgetQueryResponse, WidgetQueryRow,
        WidgetQuerySource, WidgetRowKey, WidgetUseCases, WidgetValueRow,
    };
    use crate::widget::{
        WidgetAggregation, WidgetBy, WidgetDefinition, WidgetMetric, WidgetPeriod, WidgetQuery,
        WidgetTime, WidgetVisualization, WidgetVisualizationOrder,
    };

    struct FakeWidgetQuerySource;

    #[async_trait]
    impl WidgetQuerySource for FakeWidgetQuerySource {
        async fn resolve_as_of(
            &self,
            requested: Option<NaiveDate>,
        ) -> Result<Option<NaiveDate>, WidgetQueryError> {
            Ok(requested.or_else(|| NaiveDate::from_ymd_opt(2098, 1, 5)))
        }

        async fn query(
            &self,
            _as_of: NaiveDate,
            definition: &WidgetQuery,
        ) -> Result<Vec<WidgetQueryRow>, WidgetQueryError> {
            Ok(match (definition.metric, definition.by) {
                (WidgetMetric::Change, WidgetBy::None) => vec![WidgetQueryRow {
                    key: WidgetRowKey::Market,
                    name: "全体".to_string(),
                    value: Decimal::new(-2, 2),
                }],
                (WidgetMetric::Change, WidgetBy::Group) => vec![
                    WidgetQueryRow {
                        key: WidgetRowKey::Group {
                            axis: "demo-axis".to_string(),
                            key: "group-alpha".to_string(),
                        },
                        name: "Group Alpha".to_string(),
                        value: Decimal::new(3, 2),
                    },
                    WidgetQueryRow {
                        key: WidgetRowKey::Group {
                            axis: "demo-axis".to_string(),
                            key: "group-beta".to_string(),
                        },
                        name: "Group Beta".to_string(),
                        value: Decimal::new(-4, 2),
                    },
                ],
                (WidgetMetric::Change, WidgetBy::Stock) => vec![
                    WidgetQueryRow {
                        key: WidgetRowKey::Stock("D001".to_string()),
                        name: "Stock Alpha".to_string(),
                        value: Decimal::new(3, 2),
                    },
                    WidgetQueryRow {
                        key: WidgetRowKey::Stock("D002".to_string()),
                        name: "Stock Beta".to_string(),
                        value: Decimal::new(-4, 2),
                    },
                    WidgetQueryRow {
                        key: WidgetRowKey::Stock("D003".to_string()),
                        name: "Stock Gamma".to_string(),
                        value: Decimal::new(1, 2),
                    },
                ],
                (WidgetMetric::MktCap, WidgetBy::Group) => vec![WidgetQueryRow {
                    key: WidgetRowKey::Group {
                        axis: "demo-axis".to_string(),
                        key: "group-alpha".to_string(),
                    },
                    name: "Group Alpha".to_string(),
                    value: Decimal::new(1, 2),
                }],
                _ => Vec::new(),
            })
        }
    }

    fn change_query(by: WidgetBy, axis: Option<&str>) -> WidgetQuery {
        WidgetQuery {
            metric: WidgetMetric::Change,
            period: Some(WidgetPeriod::OneDay),
            by,
            axis: axis.map(str::to_string),
            indicator: None,
            universe: None,
            agg: (by != WidgetBy::Stock).then_some(WidgetAggregation::MarketCapWeighted),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn broadcasts_market_values_and_intersects_group_rows() {
        let definition = WidgetDefinition {
            queries: BTreeMap::from([
                ("market".to_string(), change_query(WidgetBy::None, None)),
                (
                    "other".to_string(),
                    WidgetQuery {
                        metric: WidgetMetric::MktCap,
                        period: None,
                        by: WidgetBy::Group,
                        axis: Some("demo-axis".to_string()),
                        indicator: None,
                        universe: None,
                        agg: Some(WidgetAggregation::Sum),
                    },
                ),
                (
                    "sector".to_string(),
                    change_query(WidgetBy::Group, Some("demo-axis")),
                ),
            ]),
            formulas: BTreeMap::from([
                ("relative".to_string(), "sector - market".to_string()),
                ("spread".to_string(), "sector - other".to_string()),
            ]),
            viz: WidgetVisualization::Treemap {
                size: "sector".to_string(),
                color: "relative".to_string(),
            },
            time: WidgetTime {
                as_of: Some(NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date")),
                range: None,
            },
        };
        let source = FakeWidgetQuerySource;
        let result = WidgetUseCases::new(Arc::new(source) as SharedWidgetQuerySource)
            .query(definition)
            .await
            .expect("calculate widget");

        assert_eq!(
            result,
            WidgetQueryResponse {
                as_of: NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date"),
                rows: vec![
                    WidgetValueRow {
                        key: WidgetRowKey::Group {
                            axis: "demo-axis".to_string(),
                            key: "group-alpha".to_string()
                        },
                        name: "Group Alpha".to_string(),
                        values: BTreeMap::from([
                            ("market".to_string(), Some(Decimal::new(-2, 2))),
                            ("other".to_string(), Some(Decimal::new(1, 2))),
                            ("relative".to_string(), Some(Decimal::new(5, 2))),
                            ("sector".to_string(), Some(Decimal::new(3, 2))),
                            ("spread".to_string(), Some(Decimal::new(2, 2))),
                        ]),
                    },
                    WidgetValueRow {
                        key: WidgetRowKey::Group {
                            axis: "demo-axis".to_string(),
                            key: "group-beta".to_string()
                        },
                        name: "Group Beta".to_string(),
                        values: BTreeMap::from([
                            ("market".to_string(), Some(Decimal::new(-2, 2))),
                            ("relative".to_string(), Some(Decimal::new(-2, 2))),
                            ("sector".to_string(), Some(Decimal::new(-4, 2))),
                        ]),
                    },
                ],
            },
        );
    }

    #[tokio::test]
    async fn toplist_sorts_by_value_and_applies_its_limit() {
        let definition = WidgetDefinition {
            queries: BTreeMap::from([("change".to_string(), change_query(WidgetBy::Stock, None))]),
            formulas: BTreeMap::new(),
            viz: WidgetVisualization::Toplist {
                value: "change".to_string(),
                order: WidgetVisualizationOrder::Desc,
                limit: 2,
            },
            time: WidgetTime::default(),
        };
        let result =
            WidgetUseCases::new(Arc::new(FakeWidgetQuerySource) as SharedWidgetQuerySource)
                .query(definition)
                .await
                .expect("calculate toplist");

        assert_eq!(
            result,
            WidgetQueryResponse {
                as_of: NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date"),
                rows: vec![
                    WidgetValueRow {
                        key: WidgetRowKey::Stock("D001".to_string()),
                        name: "Stock Alpha".to_string(),
                        values: BTreeMap::from([("change".to_string(), Some(Decimal::new(3, 2)))]),
                    },
                    WidgetValueRow {
                        key: WidgetRowKey::Stock("D003".to_string()),
                        name: "Stock Gamma".to_string(),
                        values: BTreeMap::from([("change".to_string(), Some(Decimal::new(1, 2)))]),
                    },
                ],
            },
        );
    }
}
