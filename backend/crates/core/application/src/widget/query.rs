use std::{
    collections::{BTreeMap, btree_map::Entry},
    sync::Arc,
};

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::stock_group::{StockGroupMembership, StockGroupUseCaseError, StockGroupUseCases};

use super::{
    formula::FormulaExpression,
    types::{
        WidgetBy, WidgetDefinition, WidgetQuery, WidgetVisualization, WidgetVisualizationOrder,
    },
    validation::{
        QueryDimension, WidgetValidationError, WidgetValidationErrors, query_dimension,
        validate_widget,
    },
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
    pub classification: Option<WidgetRowClassification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetRowClassification {
    pub axis: String,
    pub group: Option<WidgetGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetGroup {
    pub key: String,
    pub name: String,
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
    #[error(transparent)]
    StockGroups(#[from] StockGroupUseCaseError),
    #[error("amount aggregation across multiple stocks requires confirmed currencies")]
    IncompatibleCurrencies,
}

#[async_trait]
pub trait WidgetQuerySource: Send + Sync {
    async fn resolve_as_of(
        &self,
        requested: Option<NaiveDate>,
    ) -> Result<Option<NaiveDate>, WidgetQueryError>;

    async fn resolve_as_of_for_queries(
        &self,
        requested: Option<NaiveDate>,
        _queries: &[WidgetQuery],
    ) -> Result<Option<NaiveDate>, WidgetQueryError> {
        self.resolve_as_of(requested).await
    }

    async fn query(
        &self,
        as_of: NaiveDate,
        definition: &WidgetQuery,
    ) -> Result<Vec<WidgetQueryRow>, WidgetQueryError>;
}

#[async_trait]
pub trait WidgetGroupMembershipSource: Send + Sync {
    async fn list_memberships(
        &self,
        stock_ids: &[String],
        axis_key: &str,
    ) -> Result<Vec<StockGroupMembership>, WidgetQueryError>;
}

#[async_trait]
impl WidgetGroupMembershipSource for StockGroupUseCases {
    async fn list_memberships(
        &self,
        stock_ids: &[String],
        axis_key: &str,
    ) -> Result<Vec<StockGroupMembership>, WidgetQueryError> {
        let axis_keys = [axis_key.to_owned()];
        self.list_memberships(stock_ids, Some(&axis_keys))
            .await
            .map_err(WidgetQueryError::from)
    }
}

pub type SharedWidgetQuerySource = Arc<dyn WidgetQuerySource + Send + Sync>;
pub type SharedWidgetGroupMembershipSource = Arc<dyn WidgetGroupMembershipSource + Send + Sync>;

#[derive(Debug, Error)]
pub enum WidgetUseCaseError {
    #[error("{0}")]
    Validation(#[from] WidgetValidationErrors),
    #[error("no widget data is available on or before the requested date")]
    NoData,
    #[error(transparent)]
    Query(#[from] WidgetQueryError),
}

#[derive(Clone)]
pub struct WidgetUseCases {
    query_source: SharedWidgetQuerySource,
    group_membership_source: SharedWidgetGroupMembershipSource,
}

impl WidgetUseCases {
    pub fn new(
        query_source: SharedWidgetQuerySource,
        group_membership_source: SharedWidgetGroupMembershipSource,
    ) -> Self {
        Self {
            query_source,
            group_membership_source,
        }
    }

    pub async fn query(
        &self,
        definition: WidgetDefinition,
    ) -> Result<WidgetQueryResponse, WidgetUseCaseError> {
        let parsed_formulas = validate_widget(&definition)?;
        let queries = definition.queries.values().cloned().collect::<Vec<_>>();
        let as_of = self
            .query_source
            .resolve_as_of_for_queries(definition.time.as_of, &queries)
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
                        classification: None,
                    })
                    .values
                    .insert(name.clone(), Some(row.value));
                values.insert(row.key, (row.name, row.value));
            }
            query_values.insert(name.as_str(), values);
        }

        broadcast_market_values(&definition, &query_values, &mut result_rows);
        evaluate_formulas(
            &definition,
            &parsed_formulas,
            &query_values,
            &mut result_rows,
        );

        let mut rows = project_visualization_rows(
            result_rows.into_values().collect(),
            &definition.viz,
            table_has_non_market_columns(&definition, &parsed_formulas),
        );

        if let WidgetVisualization::Treemap {
            hierarchy: Some(hierarchy),
            ..
        } = &definition.viz
        {
            let stock_ids = rows
                .iter()
                .filter_map(|row| match &row.key {
                    WidgetRowKey::Stock(stock_id) => Some(stock_id.clone()),
                    WidgetRowKey::Market | WidgetRowKey::Group { .. } => None,
                })
                .collect::<Vec<_>>();
            self.attach_classifications(&mut rows, &stock_ids, &hierarchy.axis)
                .await?;
        }

        Ok(WidgetQueryResponse { as_of, rows })
    }

    async fn attach_classifications(
        &self,
        rows: &mut [WidgetValueRow],
        stock_ids: &[String],
        axis_key: &str,
    ) -> Result<(), WidgetUseCaseError> {
        let memberships = self
            .group_membership_source
            .list_memberships(stock_ids, axis_key)
            .await?;
        let mut classifications = BTreeMap::new();
        for membership in memberships {
            match classifications.entry(membership.stock_id.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(membership);
                }
                Entry::Occupied(entry) => {
                    return Err(WidgetUseCaseError::Validation(WidgetValidationErrors(
                        vec![WidgetValidationError {
                            path: "viz.hierarchy.axis".to_owned(),
                            message: format!(
                                "stock `{}` belongs to multiple groups in axis `{axis_key}`",
                                entry.key()
                            ),
                        }],
                    )));
                }
            }
        }

        for row in rows {
            let WidgetRowKey::Stock(stock_id) = &row.key else {
                continue;
            };
            row.classification = Some(WidgetRowClassification {
                axis: axis_key.to_owned(),
                group: classifications
                    .remove(stock_id)
                    .map(|membership| WidgetGroup {
                        key: membership.group_key,
                        name: membership.group_name,
                    }),
            });
        }
        Ok(())
    }
}

type QueryValues<'a> = BTreeMap<&'a str, BTreeMap<WidgetRowKey, (String, Decimal)>>;

fn table_has_non_market_columns(
    definition: &WidgetDefinition,
    parsed_formulas: &BTreeMap<&str, FormulaExpression>,
) -> bool {
    let WidgetVisualization::Table { columns } = &definition.viz else {
        return false;
    };

    columns.iter().any(|column| {
        query_dimension(column, definition, parsed_formulas)
            .is_some_and(|dimension| !matches!(dimension, QueryDimension::Market))
    })
}

fn broadcast_market_values(
    definition: &WidgetDefinition,
    query_values: &QueryValues<'_>,
    result_rows: &mut BTreeMap<WidgetRowKey, WidgetValueRow>,
) {
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
        for (key, row) in result_rows.iter_mut() {
            if key != &WidgetRowKey::Market {
                row.values.insert(name.clone(), Some(value));
            }
        }
    }
}

fn evaluate_formulas<'a>(
    definition: &'a WidgetDefinition,
    parsed_formulas: &BTreeMap<&'a str, FormulaExpression>,
    query_values: &QueryValues<'a>,
    result_rows: &mut BTreeMap<WidgetRowKey, WidgetValueRow>,
) {
    for name in definition.formulas.keys() {
        let Some(expression) = parsed_formulas.get(name.as_str()) else {
            continue;
        };
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
            None => result_rows.keys().cloned().collect(),
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

            let value = expression.evaluate(&|reference| referenced_values.get(reference).copied());
            result_row.values.insert(name.clone(), value);
        }
    }
}

fn project_visualization_rows(
    mut rows: Vec<WidgetValueRow>,
    visualization: &WidgetVisualization,
    table_has_non_market_columns: bool,
) -> Vec<WidgetValueRow> {
    match visualization {
        WidgetVisualization::Treemap { size, color, .. } => rows
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
            ..
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
        WidgetVisualization::Table { columns } => {
            rows.retain_mut(|row| {
                if table_has_non_market_columns && row.key == WidgetRowKey::Market {
                    return false;
                }
                row.values.retain(|name, _| columns.contains(name));
                columns.iter().any(|name| row.values.contains_key(name))
            });
            rows
        }
        WidgetVisualization::Line { .. } => Vec::new(),
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
        SharedWidgetGroupMembershipSource, SharedWidgetQuerySource, WidgetGroupMembershipSource,
        WidgetQueryError, WidgetQueryResponse, WidgetQueryRow, WidgetQuerySource,
        WidgetRowClassification, WidgetRowKey, WidgetUseCases, WidgetValueRow,
    };
    use crate::stock_group::StockGroupMembership;
    use crate::widget::{
        WidgetAggregation, WidgetBy, WidgetDefinition, WidgetGroup, WidgetHierarchy, WidgetMetric,
        WidgetPeriod, WidgetQuery, WidgetTime, WidgetVisualization, WidgetVisualizationOrder,
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
                        key: WidgetRowKey::Stock("DEMO-A".to_string()),
                        name: "Demo Stock A".to_string(),
                        value: Decimal::new(3, 2),
                    },
                    WidgetQueryRow {
                        key: WidgetRowKey::Stock("DEMO-B".to_string()),
                        name: "Demo Stock B".to_string(),
                        value: Decimal::new(-4, 2),
                    },
                    WidgetQueryRow {
                        key: WidgetRowKey::Stock("DEMO-C".to_string()),
                        name: "Demo Stock C".to_string(),
                        value: Decimal::new(1, 2),
                    },
                ],
                (WidgetMetric::MktCap, WidgetBy::Stock) => vec![
                    WidgetQueryRow {
                        key: WidgetRowKey::Stock("DEMO-A".to_string()),
                        name: "Demo Stock A".to_string(),
                        value: Decimal::new(120, 0),
                    },
                    WidgetQueryRow {
                        key: WidgetRowKey::Stock("DEMO-B".to_string()),
                        name: "Demo Stock B".to_string(),
                        value: Decimal::new(80, 0),
                    },
                    WidgetQueryRow {
                        key: WidgetRowKey::Stock("DEMO-C".to_string()),
                        name: "Demo Stock C".to_string(),
                        value: Decimal::new(40, 0),
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

    struct FakeWidgetGroupMembershipSource {
        memberships: Vec<StockGroupMembership>,
    }

    #[async_trait]
    impl WidgetGroupMembershipSource for FakeWidgetGroupMembershipSource {
        async fn list_memberships(
            &self,
            stock_ids: &[String],
            axis_key: &str,
        ) -> Result<Vec<StockGroupMembership>, WidgetQueryError> {
            Ok(self
                .memberships
                .iter()
                .filter(|membership| {
                    membership.axis_key == axis_key && stock_ids.contains(&membership.stock_id)
                })
                .cloned()
                .collect())
        }
    }

    fn fake_memberships(
        memberships: Vec<StockGroupMembership>,
    ) -> SharedWidgetGroupMembershipSource {
        Arc::new(FakeWidgetGroupMembershipSource { memberships })
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
                (
                    "arithmetic".to_string(),
                    "(-sector + market) * 2 / 2".to_string(),
                ),
                ("division_by_zero".to_string(), "market / 0".to_string()),
                ("market_level".to_string(), "market * 2".to_string()),
                ("relative".to_string(), "sector - market".to_string()),
                ("spread".to_string(), "sector - other".to_string()),
            ]),
            display: BTreeMap::new(),
            viz: WidgetVisualization::Treemap {
                size: "sector".to_string(),
                color: "market_level".to_string(),
                hierarchy: None,
                color_scale: None,
            },
            time: WidgetTime {
                as_of: Some(NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date")),
                range: None,
            },
        };
        let source = FakeWidgetQuerySource;
        let result = WidgetUseCases::new(
            Arc::new(source) as SharedWidgetQuerySource,
            fake_memberships(Vec::new()),
        )
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
                            ("arithmetic".to_string(), Some(Decimal::new(-5, 2))),
                            ("division_by_zero".to_string(), None),
                            ("market".to_string(), Some(Decimal::new(-2, 2))),
                            ("market_level".to_string(), Some(Decimal::new(-4, 2))),
                            ("other".to_string(), Some(Decimal::new(1, 2))),
                            ("relative".to_string(), Some(Decimal::new(5, 2))),
                            ("sector".to_string(), Some(Decimal::new(3, 2))),
                            ("spread".to_string(), Some(Decimal::new(2, 2))),
                        ]),
                        classification: None,
                    },
                    WidgetValueRow {
                        key: WidgetRowKey::Group {
                            axis: "demo-axis".to_string(),
                            key: "group-beta".to_string()
                        },
                        name: "Group Beta".to_string(),
                        values: BTreeMap::from([
                            ("arithmetic".to_string(), Some(Decimal::new(2, 2))),
                            ("division_by_zero".to_string(), None),
                            ("market".to_string(), Some(Decimal::new(-2, 2))),
                            ("market_level".to_string(), Some(Decimal::new(-4, 2))),
                            ("relative".to_string(), Some(Decimal::new(-2, 2))),
                            ("sector".to_string(), Some(Decimal::new(-4, 2))),
                        ]),
                        classification: None,
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
            display: BTreeMap::new(),
            viz: WidgetVisualization::Toplist {
                value: "change".to_string(),
                order: WidgetVisualizationOrder::Desc,
                limit: 2,
                bar_scale: None,
            },
            time: WidgetTime::default(),
        };
        let result = WidgetUseCases::new(
            Arc::new(FakeWidgetQuerySource) as SharedWidgetQuerySource,
            fake_memberships(Vec::new()),
        )
        .query(definition)
        .await
        .expect("calculate toplist");

        assert_eq!(
            result,
            WidgetQueryResponse {
                as_of: NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date"),
                rows: vec![
                    WidgetValueRow {
                        key: WidgetRowKey::Stock("DEMO-A".to_string()),
                        name: "Demo Stock A".to_string(),
                        values: BTreeMap::from([("change".to_string(), Some(Decimal::new(3, 2)))]),
                        classification: None,
                    },
                    WidgetValueRow {
                        key: WidgetRowKey::Stock("DEMO-C".to_string()),
                        name: "Demo Stock C".to_string(),
                        values: BTreeMap::from([("change".to_string(), Some(Decimal::new(1, 2)))]),
                        classification: None,
                    },
                ],
            },
        );
    }

    #[tokio::test]
    async fn table_keeps_requested_values_and_formula_nulls() {
        let definition = WidgetDefinition {
            queries: BTreeMap::from([("change".to_string(), change_query(WidgetBy::Stock, None))]),
            formulas: BTreeMap::from([
                ("double".to_string(), "change * 2".to_string()),
                ("zero".to_string(), "change / 0".to_string()),
            ]),
            display: BTreeMap::new(),
            viz: WidgetVisualization::Table {
                columns: vec![
                    "zero".to_string(),
                    "double".to_string(),
                    "change".to_string(),
                ],
            },
            time: WidgetTime::default(),
        };
        let result = WidgetUseCases::new(
            Arc::new(FakeWidgetQuerySource) as SharedWidgetQuerySource,
            fake_memberships(Vec::new()),
        )
        .query(definition)
        .await
        .expect("calculate table widget");

        assert_eq!(
            result,
            WidgetQueryResponse {
                as_of: NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date"),
                rows: vec![
                    WidgetValueRow {
                        key: WidgetRowKey::Stock("DEMO-A".to_string()),
                        name: "Demo Stock A".to_string(),
                        values: BTreeMap::from([
                            ("change".to_string(), Some(Decimal::new(3, 2))),
                            ("double".to_string(), Some(Decimal::new(6, 2))),
                            ("zero".to_string(), None),
                        ]),
                        classification: None,
                    },
                    WidgetValueRow {
                        key: WidgetRowKey::Stock("DEMO-B".to_string()),
                        name: "Demo Stock B".to_string(),
                        values: BTreeMap::from([
                            ("change".to_string(), Some(Decimal::new(-4, 2))),
                            ("double".to_string(), Some(Decimal::new(-8, 2))),
                            ("zero".to_string(), None),
                        ]),
                        classification: None,
                    },
                    WidgetValueRow {
                        key: WidgetRowKey::Stock("DEMO-C".to_string()),
                        name: "Demo Stock C".to_string(),
                        values: BTreeMap::from([
                            ("change".to_string(), Some(Decimal::new(1, 2))),
                            ("double".to_string(), Some(Decimal::new(2, 2))),
                            ("zero".to_string(), None),
                        ]),
                        classification: None,
                    },
                ],
            },
        );
    }

    fn membership(stock_id: &str, group_key: &str, group_name: &str) -> StockGroupMembership {
        StockGroupMembership {
            axis_key: "demo-axis".to_string(),
            axis_name: "Demo Axis".to_string(),
            group_key: group_key.to_string(),
            group_name: group_name.to_string(),
            stock_id: stock_id.to_string(),
        }
    }

    fn hierarchical_definition() -> WidgetDefinition {
        WidgetDefinition {
            queries: BTreeMap::from([
                (
                    "size".to_string(),
                    WidgetQuery {
                        metric: WidgetMetric::MktCap,
                        period: None,
                        by: WidgetBy::Stock,
                        axis: None,
                        indicator: None,
                        universe: None,
                        agg: None,
                    },
                ),
                ("change".to_string(), change_query(WidgetBy::Stock, None)),
            ]),
            formulas: BTreeMap::new(),
            display: BTreeMap::new(),
            viz: WidgetVisualization::Treemap {
                size: "size".to_string(),
                color: "change".to_string(),
                hierarchy: Some(WidgetHierarchy {
                    axis: "demo-axis".to_string(),
                }),
                color_scale: None,
            },
            time: WidgetTime::default(),
        }
    }

    #[tokio::test]
    async fn hierarchical_treemap_attaches_memberships_and_keeps_unassigned_stocks() {
        let result = WidgetUseCases::new(
            Arc::new(FakeWidgetQuerySource) as SharedWidgetQuerySource,
            fake_memberships(vec![
                membership("DEMO-A", "group-alpha", "Group Alpha"),
                membership("DEMO-B", "group-alpha", "Group Alpha"),
            ]),
        )
        .query(hierarchical_definition())
        .await
        .expect("calculate hierarchical treemap");

        assert_eq!(
            result,
            WidgetQueryResponse {
                as_of: NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date"),
                rows: vec![
                    WidgetValueRow {
                        key: WidgetRowKey::Stock("DEMO-A".to_string()),
                        name: "Demo Stock A".to_string(),
                        values: BTreeMap::from([
                            ("change".to_string(), Some(Decimal::new(3, 2))),
                            ("size".to_string(), Some(Decimal::new(120, 0))),
                        ]),
                        classification: Some(WidgetRowClassification {
                            axis: "demo-axis".to_string(),
                            group: Some(WidgetGroup {
                                key: "group-alpha".to_string(),
                                name: "Group Alpha".to_string(),
                            }),
                        }),
                    },
                    WidgetValueRow {
                        key: WidgetRowKey::Stock("DEMO-B".to_string()),
                        name: "Demo Stock B".to_string(),
                        values: BTreeMap::from([
                            ("change".to_string(), Some(Decimal::new(-4, 2))),
                            ("size".to_string(), Some(Decimal::new(80, 0))),
                        ]),
                        classification: Some(WidgetRowClassification {
                            axis: "demo-axis".to_string(),
                            group: Some(WidgetGroup {
                                key: "group-alpha".to_string(),
                                name: "Group Alpha".to_string(),
                            }),
                        }),
                    },
                    WidgetValueRow {
                        key: WidgetRowKey::Stock("DEMO-C".to_string()),
                        name: "Demo Stock C".to_string(),
                        values: BTreeMap::from([
                            ("change".to_string(), Some(Decimal::new(1, 2))),
                            ("size".to_string(), Some(Decimal::new(40, 0))),
                        ]),
                        classification: Some(WidgetRowClassification {
                            axis: "demo-axis".to_string(),
                            group: None,
                        }),
                    },
                ],
            },
        );
    }

    #[tokio::test]
    async fn hierarchical_treemap_rejects_multiple_memberships_on_its_axis() {
        let error = WidgetUseCases::new(
            Arc::new(FakeWidgetQuerySource) as SharedWidgetQuerySource,
            fake_memberships(vec![
                membership("DEMO-A", "group-alpha", "Group Alpha"),
                membership("DEMO-A", "group-beta", "Group Beta"),
            ]),
        )
        .query(hierarchical_definition())
        .await
        .expect_err("multiple memberships must be rejected");

        assert_eq!(
            error.to_string(),
            "viz.hierarchy.axis: stock `DEMO-A` belongs to multiple groups in axis `demo-axis`",
        );
    }
}
