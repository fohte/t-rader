use std::collections::{BTreeMap, BTreeSet};

use super::{
    formula::{FormulaExpression, parse_formula},
    types::{
        WidgetAggregation, WidgetBy, WidgetDefinition, WidgetMetric, WidgetQuery, WidgetUniverse,
        WidgetVisualization,
    },
};

#[cfg(test)]
use super::types::{WidgetPeriod, WidgetTime, WidgetVisualizationOrder};

impl WidgetDefinition {
    pub fn validate(&self) -> Result<(), WidgetValidationErrors> {
        validate_widget(self).map(|_| ())
    }
}

pub(super) fn validate_widget(
    definition: &WidgetDefinition,
) -> Result<BTreeMap<&str, FormulaExpression>, WidgetValidationErrors> {
    let mut errors = Vec::new();

    if definition.queries.is_empty() {
        errors.push(WidgetValidationError::new(
            "queries",
            "at least one query is required",
        ));
    }

    let mut names = BTreeSet::new();
    for (name, query) in &definition.queries {
        if !is_identifier(name) {
            errors.push(WidgetValidationError::new(
                    format!("queries.{name}"),
                    "name must contain only ASCII letters, digits, and underscores, and start with a letter or underscore",
                ));
        }
        names.insert(name.as_str());
        validate_query(name, query, &mut errors);
    }

    let mut parsed_formulas = BTreeMap::new();
    for (name, formula) in &definition.formulas {
        if !is_identifier(name) {
            errors.push(WidgetValidationError::new(
                    format!("formulas.{name}"),
                    "name must contain only ASCII letters, digits, and underscores, and start with a letter or underscore",
                ));
        }
        if names.contains(name.as_str()) {
            errors.push(WidgetValidationError::new(
                format!("formulas.{name}"),
                "formula name conflicts with a query name",
            ));
        }
        match parse_formula(formula) {
            Ok(expression) => {
                validate_formula_references(name, &expression, definition, &mut errors);
                parsed_formulas.insert(name.as_str(), expression);
            }
            Err(message) => errors.push(WidgetValidationError::new(
                format!("formulas.{name}"),
                format!("invalid expression: {message}"),
            )),
        }
    }

    match &definition.viz {
        WidgetVisualization::Treemap {
            size,
            color,
            hierarchy,
            color_scale,
        } => {
            validate_value_reference("viz.size", size, definition, &mut errors);
            validate_value_reference("viz.color", color, definition, &mut errors);
            validate_matching_dimensions(
                "viz",
                [size.as_str(), color.as_str()],
                definition,
                &parsed_formulas,
                &mut errors,
            );
            if let Some(hierarchy) = hierarchy {
                if hierarchy.axis.trim().is_empty() {
                    errors.push(WidgetValidationError::new(
                        "viz.hierarchy.axis",
                        "axis must not be empty",
                    ));
                }
                for name in [size.as_str(), color.as_str()] {
                    if query_dimension(name, definition, &parsed_formulas)
                        != Some(QueryDimension::Stock)
                    {
                        errors.push(WidgetValidationError::new(
                            "viz.hierarchy",
                            "hierarchy requires values that use stock rows",
                        ));
                        break;
                    }
                }
            }
            if let Some(scale) = color_scale {
                validate_scale("viz.color_scale", scale, &mut errors);
            }
        }
        WidgetVisualization::Toplist {
            value,
            limit,
            bar_scale,
            ..
        } => {
            validate_value_reference("viz.value", value, definition, &mut errors);
            if *limit == 0 {
                errors.push(WidgetValidationError::new(
                    "viz.limit",
                    "limit must be greater than zero",
                ));
            }
            if let Some(scale) = bar_scale {
                validate_scale("viz.bar_scale", scale, &mut errors);
            }
        }
        WidgetVisualization::Table { .. } => errors.push(WidgetValidationError::new(
            "viz.type",
            "table visualization is not supported yet",
        )),
        WidgetVisualization::Line { .. } => errors.push(WidgetValidationError::new(
            "viz.type",
            "line visualization is not supported yet",
        )),
    }

    for (name, display) in &definition.display {
        if !definition.queries.contains_key(name) && !definition.formulas.contains_key(name) {
            errors.push(WidgetValidationError::new(
                format!("display.{name}"),
                "name must reference a query or formula",
            ));
        }
        if display
            .label
            .as_deref()
            .is_some_and(|label| label.trim().is_empty())
        {
            errors.push(WidgetValidationError::new(
                format!("display.{name}.label"),
                "label must not be empty",
            ));
        }
    }

    if definition.time.range.is_some() {
        errors.push(WidgetValidationError::new(
            "time.range",
            "time ranges are not supported yet",
        ));
    }

    if errors.is_empty() {
        Ok(parsed_formulas)
    } else {
        Err(WidgetValidationErrors(errors))
    }
}

fn validate_scale(
    path: &str,
    scale: &super::types::WidgetScale,
    errors: &mut Vec<WidgetValidationError>,
) {
    if scale.min >= scale.max {
        errors.push(WidgetValidationError::new(
            path,
            "min must be less than max",
        ));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetValidationError {
    pub path: String,
    pub message: String,
}

impl WidgetValidationError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetValidationErrors(pub Vec<WidgetValidationError>);

impl std::fmt::Display for WidgetValidationErrors {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = self
            .0
            .iter()
            .map(|error| format!("{}: {}", error.path, error.message))
            .collect::<Vec<_>>()
            .join("; ");
        formatter.write_str(&message)
    }
}

impl std::error::Error for WidgetValidationErrors {}

fn validate_query(name: &str, query: &WidgetQuery, errors: &mut Vec<WidgetValidationError>) {
    if !matches!(query.metric, WidgetMetric::Change | WidgetMetric::MktCap) {
        errors.push(WidgetValidationError::new(
            format!("queries.{name}.metric"),
            format!("metric `{}` is not supported yet", query.metric.as_str()),
        ));
    }

    match query.metric {
        WidgetMetric::Change if query.period.is_none() => errors.push(WidgetValidationError::new(
            format!("queries.{name}.period"),
            "period is required for change metrics",
        )),
        WidgetMetric::MktCap if query.period.is_some() => errors.push(WidgetValidationError::new(
            format!("queries.{name}.period"),
            "period is only valid for change metrics",
        )),
        _ => {}
    }

    match (query.by, query.axis.as_deref()) {
        (WidgetBy::Group, None | Some("")) => errors.push(WidgetValidationError::new(
            format!("queries.{name}.axis"),
            "axis is required when by is group",
        )),
        (WidgetBy::Group, Some(_)) => {}
        (_, Some(_)) => errors.push(WidgetValidationError::new(
            format!("queries.{name}.axis"),
            "axis is only valid when by is group",
        )),
        (_, None) => {}
    }

    match (query.by, query.agg) {
        (WidgetBy::Stock, Some(_)) => errors.push(WidgetValidationError::new(
            format!("queries.{name}.agg"),
            "agg is not valid when by is stock",
        )),
        (WidgetBy::Stock, None) => {}
        (_, None) => errors.push(WidgetValidationError::new(
            format!("queries.{name}.agg"),
            "agg is required when by is none or group",
        )),
        (_, Some(WidgetAggregation::MarketCapWeighted)) if query.metric != WidgetMetric::Change => {
            errors.push(WidgetValidationError::new(
                format!("queries.{name}.agg"),
                "mkt_cap_weighted is only valid for change metrics",
            ));
        }
        (_, Some(_)) => {}
    }

    match &query.universe {
        Some(WidgetUniverse::Group { group }) => {
            let Some((axis, key)) = group.split_once('/') else {
                errors.push(WidgetValidationError::new(
                    format!("queries.{name}.universe.group"),
                    "group must use the `<axis>/<key>` format",
                ));
                validate_indicator(name, query, errors);
                return;
            };
            if axis.is_empty() || key.is_empty() || group.matches('/').count() != 1 {
                errors.push(WidgetValidationError::new(
                    format!("queries.{name}.universe.group"),
                    "group must use the `<axis>/<key>` format",
                ));
            }
        }
        Some(WidgetUniverse::Stocks { stocks }) if stocks.is_empty() => {
            errors.push(WidgetValidationError::new(
                format!("queries.{name}.universe.stocks"),
                "stocks must contain at least one stock id",
            ));
        }
        Some(WidgetUniverse::Stocks { stocks })
            if stocks.iter().any(|stock| stock.trim().is_empty()) =>
        {
            errors.push(WidgetValidationError::new(
                format!("queries.{name}.universe.stocks"),
                "stock ids must not be empty",
            ));
        }
        _ => {}
    }

    validate_indicator(name, query, errors);
}

fn validate_indicator(name: &str, query: &WidgetQuery, errors: &mut Vec<WidgetValidationError>) {
    if query.metric == WidgetMetric::Indicator
        && query.indicator.as_deref().unwrap_or_default().is_empty()
    {
        errors.push(WidgetValidationError::new(
            format!("queries.{name}.indicator"),
            "indicator id is required for indicator metrics",
        ));
    } else if query.metric != WidgetMetric::Indicator && query.indicator.is_some() {
        errors.push(WidgetValidationError::new(
            format!("queries.{name}.indicator"),
            "indicator is only valid for indicator metrics",
        ));
    }
}

fn validate_formula_references(
    formula_name: &str,
    expression: &FormulaExpression,
    definition: &WidgetDefinition,
    errors: &mut Vec<WidgetValidationError>,
) {
    for reference in expression.references() {
        if !definition.queries.contains_key(reference) {
            let message = if definition.formulas.contains_key(reference) {
                "formulas may reference query names only"
            } else {
                "unknown query name"
            };
            errors.push(WidgetValidationError::new(
                format!("formulas.{formula_name}"),
                format!("{message}: `{reference}`"),
            ));
        }
    }
    validate_matching_dimensions(
        &format!("formulas.{formula_name}"),
        expression.references(),
        definition,
        &BTreeMap::new(),
        errors,
    );
}

fn validate_value_reference(
    path: &str,
    reference: &str,
    definition: &WidgetDefinition,
    errors: &mut Vec<WidgetValidationError>,
) {
    if !definition.queries.contains_key(reference) && !definition.formulas.contains_key(reference) {
        errors.push(WidgetValidationError::new(
            path,
            format!("unknown query or formula name: `{reference}`"),
        ));
    }
}

fn validate_matching_dimensions<'a>(
    path: &str,
    names: impl IntoIterator<Item = &'a str>,
    definition: &WidgetDefinition,
    parsed_formulas: &BTreeMap<&str, FormulaExpression>,
    errors: &mut Vec<WidgetValidationError>,
) {
    let mut dimension: Option<QueryDimension> = None;
    for name in names {
        let Some(candidate) = query_dimension(name, definition, parsed_formulas) else {
            continue;
        };
        if matches!(candidate, QueryDimension::Market) {
            continue;
        }
        if let Some(existing) = &dimension {
            if existing != &candidate {
                errors.push(WidgetValidationError::new(
                    path,
                    "values must use the same row dimension",
                ));
                return;
            }
        } else {
            dimension = Some(candidate);
        }
    }
}

fn query_dimension(
    name: &str,
    definition: &WidgetDefinition,
    parsed_formulas: &BTreeMap<&str, FormulaExpression>,
) -> Option<QueryDimension> {
    if let Some(query) = definition.queries.get(name) {
        return match query.by {
            WidgetBy::None => Some(QueryDimension::Market),
            WidgetBy::Stock => Some(QueryDimension::Stock),
            WidgetBy::Group => query.axis.clone().map(QueryDimension::Group),
        };
    }

    let expression = parsed_formulas.get(name)?;
    let mut dimension = None;
    for reference in expression.references() {
        let query = definition.queries.get(reference)?;
        let candidate = match query.by {
            WidgetBy::None => QueryDimension::Market,
            WidgetBy::Stock => QueryDimension::Stock,
            WidgetBy::Group => QueryDimension::Group(query.axis.clone()?),
        };
        if matches!(candidate, QueryDimension::Market) {
            continue;
        }
        if dimension
            .as_ref()
            .is_some_and(|existing| existing != &candidate)
        {
            return None;
        }
        dimension = Some(candidate);
    }
    Some(dimension.unwrap_or(QueryDimension::Market))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum QueryDimension {
    Market,
    Stock,
    Group(String),
}

fn is_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::NaiveDate;
    use indoc::indoc;
    use rstest::rstest;
    use serde_json::json;

    use super::super::types::{WidgetHierarchy, WidgetScale, WidgetValueDisplay};
    use super::*;

    fn definition() -> WidgetDefinition {
        WidgetDefinition {
            queries: BTreeMap::from([
                (
                    "sector".to_string(),
                    WidgetQuery {
                        metric: WidgetMetric::Change,
                        period: Some(WidgetPeriod::OneDay),
                        by: WidgetBy::Group,
                        axis: Some("demo-axis".to_string()),
                        indicator: None,
                        universe: None,
                        agg: Some(WidgetAggregation::MarketCapWeighted),
                    },
                ),
                (
                    "market".to_string(),
                    WidgetQuery {
                        metric: WidgetMetric::Change,
                        period: Some(WidgetPeriod::OneDay),
                        by: WidgetBy::None,
                        axis: None,
                        indicator: None,
                        universe: None,
                        agg: Some(WidgetAggregation::MarketCapWeighted),
                    },
                ),
            ]),
            formulas: BTreeMap::from([("relative".to_string(), "sector - market".to_string())]),
            display: BTreeMap::new(),
            viz: WidgetVisualization::Treemap {
                size: "market".to_string(),
                color: "relative".to_string(),
                hierarchy: None,
                color_scale: None,
            },
            time: WidgetTime {
                as_of: Some(NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date")),
                range: None,
            },
        }
    }

    #[derive(Clone, Copy)]
    enum SemanticValidationCase {
        Valid,
        UnsupportedMetric,
        UnsupportedRange,
        UnknownVisualizationValue,
        GroupAxisMissing,
        GroupAggregationMissing,
        MarketAggregationMissing,
        StockAggregationForbidden,
        MarketCapWeightedOnMarketCap,
        InvalidGroupUniverse,
        ZeroToplistLimit,
        MismatchedRowDimensions,
        FormulaQueryNameCollision,
        HierarchyRequiresStockRows,
        InvalidColorScale,
        InvalidBarScale,
        EmptyHierarchyAxis,
        EmptyDisplayLabel,
        EqualColorScaleBounds,
        EqualBarScaleBounds,
        UnknownDisplayName,
    }

    fn semantic_validation_input(case: SemanticValidationCase) -> WidgetDefinition {
        let mut widget = definition();
        match case {
            SemanticValidationCase::Valid => {}
            SemanticValidationCase::UnsupportedMetric => {
                let query = widget.queries.get_mut("market").expect("market query");
                query.metric = WidgetMetric::Per;
                query.agg = Some(WidgetAggregation::Count);
            }
            SemanticValidationCase::UnsupportedRange => {
                widget.time.range = Some("1y".to_string());
            }
            SemanticValidationCase::UnknownVisualizationValue => {
                widget.viz = WidgetVisualization::Treemap {
                    size: "fictional-missing-query".to_string(),
                    color: "relative".to_string(),
                    hierarchy: None,
                    color_scale: None,
                };
            }
            SemanticValidationCase::GroupAxisMissing => {
                widget.queries.get_mut("sector").expect("sector query").axis = None;
            }
            SemanticValidationCase::GroupAggregationMissing => {
                widget.queries.get_mut("sector").expect("sector query").agg = None;
            }
            SemanticValidationCase::MarketAggregationMissing => {
                widget.queries.get_mut("market").expect("market query").agg = None;
            }
            SemanticValidationCase::StockAggregationForbidden => {
                widget.queries.get_mut("market").expect("market query").by = WidgetBy::Stock;
                widget
                    .formulas
                    .insert("relative".to_string(), "market".to_string());
            }
            SemanticValidationCase::MarketCapWeightedOnMarketCap => {
                let query = widget.queries.get_mut("market").expect("market query");
                query.metric = WidgetMetric::MktCap;
                query.period = None;
            }
            SemanticValidationCase::InvalidGroupUniverse => {
                widget
                    .queries
                    .get_mut("sector")
                    .expect("sector query")
                    .universe = Some(WidgetUniverse::Group {
                    group: "fictional-axis".to_string(),
                });
            }
            SemanticValidationCase::ZeroToplistLimit => {
                widget.viz = WidgetVisualization::Toplist {
                    value: "market".to_string(),
                    order: WidgetVisualizationOrder::Desc,
                    limit: 0,
                    bar_scale: None,
                };
            }
            SemanticValidationCase::MismatchedRowDimensions => {
                let mut stock_query = widget.queries.get("market").expect("market query").clone();
                stock_query.by = WidgetBy::Stock;
                stock_query.agg = None;
                widget.queries.insert("stock".to_string(), stock_query);
                widget.viz = WidgetVisualization::Treemap {
                    size: "sector".to_string(),
                    color: "stock".to_string(),
                    hierarchy: None,
                    color_scale: None,
                };
            }
            SemanticValidationCase::FormulaQueryNameCollision => {
                let market_query = widget.queries.get("market").expect("market query").clone();
                widget.queries.insert("relative".to_string(), market_query);
                widget.viz = WidgetVisualization::Treemap {
                    size: "market".to_string(),
                    color: "market".to_string(),
                    hierarchy: None,
                    color_scale: None,
                };
            }
            SemanticValidationCase::HierarchyRequiresStockRows => {
                widget.viz = WidgetVisualization::Treemap {
                    size: "sector".to_string(),
                    color: "sector".to_string(),
                    hierarchy: Some(WidgetHierarchy {
                        axis: "demo-axis".to_string(),
                    }),
                    color_scale: None,
                };
            }
            SemanticValidationCase::InvalidColorScale => {
                widget.viz = WidgetVisualization::Treemap {
                    size: "market".to_string(),
                    color: "market".to_string(),
                    hierarchy: None,
                    color_scale: Some(WidgetScale {
                        min: rust_decimal::Decimal::ONE,
                        max: rust_decimal::Decimal::ZERO,
                    }),
                };
            }
            SemanticValidationCase::InvalidBarScale => {
                widget.viz = WidgetVisualization::Toplist {
                    value: "market".to_string(),
                    order: WidgetVisualizationOrder::Desc,
                    limit: 5,
                    bar_scale: Some(WidgetScale {
                        min: rust_decimal::Decimal::ONE,
                        max: rust_decimal::Decimal::ZERO,
                    }),
                };
            }
            SemanticValidationCase::EmptyHierarchyAxis => {
                for name in ["sector", "market"] {
                    let query = widget.queries.get_mut(name).expect("query");
                    query.by = WidgetBy::Stock;
                    query.axis = None;
                    query.agg = None;
                }
                widget.viz = WidgetVisualization::Treemap {
                    size: "market".to_string(),
                    color: "relative".to_string(),
                    hierarchy: Some(WidgetHierarchy {
                        axis: "  ".to_string(),
                    }),
                    color_scale: None,
                };
            }
            SemanticValidationCase::EmptyDisplayLabel => {
                widget.display.insert(
                    "market".to_string(),
                    WidgetValueDisplay {
                        label: Some("  ".to_string()),
                        format: None,
                    },
                );
            }
            SemanticValidationCase::EqualColorScaleBounds => {
                widget.viz = WidgetVisualization::Treemap {
                    size: "market".to_string(),
                    color: "market".to_string(),
                    hierarchy: None,
                    color_scale: Some(WidgetScale {
                        min: rust_decimal::Decimal::ZERO,
                        max: rust_decimal::Decimal::ZERO,
                    }),
                };
            }
            SemanticValidationCase::EqualBarScaleBounds => {
                widget.viz = WidgetVisualization::Toplist {
                    value: "market".to_string(),
                    order: WidgetVisualizationOrder::Desc,
                    limit: 5,
                    bar_scale: Some(WidgetScale {
                        min: rust_decimal::Decimal::ZERO,
                        max: rust_decimal::Decimal::ZERO,
                    }),
                };
            }
            SemanticValidationCase::UnknownDisplayName => {
                widget.display.insert(
                    "fictional-unknown-value".to_string(),
                    WidgetValueDisplay::default(),
                );
            }
        }

        widget
    }

    #[rstest]
    #[case::yaml(
        indoc! {"
            queries:
              market:
                metric: change
                period: 1d
                by: none
                agg: mkt_cap_weighted
            viz:
              type: toplist
              value: market
              order: desc
              limit: 10
        "},
        true
    )]
    #[case::json(r#"{"queries":{"market":{"metric":"change","period":"1d","by":"none","agg":"mkt_cap_weighted"}},"viz":{"type":"toplist","value":"market","order":"desc","limit":10}}"#, false)]
    fn parses_yaml_and_json_into_the_same_widget_type(#[case] input: &str, #[case] yaml: bool) {
        let parsed = if yaml {
            WidgetDefinition::from_yaml(input)
        } else {
            WidgetDefinition::from_json(input)
        }
        .expect("parse widget definition");

        let expected = WidgetDefinition {
            queries: BTreeMap::from([(
                "market".to_string(),
                WidgetQuery {
                    metric: WidgetMetric::Change,
                    period: Some(WidgetPeriod::OneDay),
                    by: WidgetBy::None,
                    axis: None,
                    indicator: None,
                    universe: None,
                    agg: Some(WidgetAggregation::MarketCapWeighted),
                },
            )]),
            formulas: BTreeMap::new(),
            display: BTreeMap::new(),
            viz: WidgetVisualization::Toplist {
                value: "market".to_string(),
                order: WidgetVisualizationOrder::Desc,
                limit: 10,
                bar_scale: None,
            },
            time: WidgetTime::default(),
        };
        assert_eq!(parsed, expected);
    }

    #[rstest]
    #[case::valid(SemanticValidationCase::Valid, None)]
    #[case::unsupported_metric(
        SemanticValidationCase::UnsupportedMetric,
        Some("queries.market.metric: metric `per` is not supported yet")
    )]
    #[case::unsupported_range(
        SemanticValidationCase::UnsupportedRange,
        Some("time.range: time ranges are not supported yet")
    )]
    #[case::unknown_viz_value(
        SemanticValidationCase::UnknownVisualizationValue,
        Some("viz.size: unknown query or formula name: `fictional-missing-query`")
    )]
    #[case::group_axis_is_required(
        SemanticValidationCase::GroupAxisMissing,
        Some("queries.sector.axis: axis is required when by is group")
    )]
    #[case::group_aggregation_is_required(
        SemanticValidationCase::GroupAggregationMissing,
        Some("queries.sector.agg: agg is required when by is none or group")
    )]
    #[case::market_aggregation_is_required(
        SemanticValidationCase::MarketAggregationMissing,
        Some("queries.market.agg: agg is required when by is none or group")
    )]
    #[case::stock_aggregation_is_forbidden(
        SemanticValidationCase::StockAggregationForbidden,
        Some("queries.market.agg: agg is not valid when by is stock")
    )]
    #[case::market_cap_weighted_requires_change(
        SemanticValidationCase::MarketCapWeightedOnMarketCap,
        Some("queries.market.agg: mkt_cap_weighted is only valid for change metrics")
    )]
    #[case::group_universe_requires_axis_and_key(
        SemanticValidationCase::InvalidGroupUniverse,
        Some("queries.sector.universe.group: group must use the `<axis>/<key>` format")
    )]
    #[case::toplist_limit_must_be_positive(
        SemanticValidationCase::ZeroToplistLimit,
        Some("viz.limit: limit must be greater than zero")
    )]
    #[case::row_dimensions_must_match(
        SemanticValidationCase::MismatchedRowDimensions,
        Some("viz: values must use the same row dimension")
    )]
    #[case::formula_name_must_not_match_query_name(
        SemanticValidationCase::FormulaQueryNameCollision,
        Some("formulas.relative: formula name conflicts with a query name")
    )]
    #[case::hierarchy_requires_stock_rows(
        SemanticValidationCase::HierarchyRequiresStockRows,
        Some("viz.hierarchy: hierarchy requires values that use stock rows")
    )]
    #[case::color_scale_requires_ordered_bounds(
        SemanticValidationCase::InvalidColorScale,
        Some("viz.color_scale: min must be less than max")
    )]
    #[case::bar_scale_requires_ordered_bounds(
        SemanticValidationCase::InvalidBarScale,
        Some("viz.bar_scale: min must be less than max")
    )]
    #[case::hierarchy_axis_must_not_be_blank(
        SemanticValidationCase::EmptyHierarchyAxis,
        Some("viz.hierarchy.axis: axis must not be empty")
    )]
    #[case::display_label_must_not_be_blank(
        SemanticValidationCase::EmptyDisplayLabel,
        Some("display.market.label: label must not be empty")
    )]
    #[case::color_scale_bounds_must_differ(
        SemanticValidationCase::EqualColorScaleBounds,
        Some("viz.color_scale: min must be less than max")
    )]
    #[case::bar_scale_bounds_must_differ(
        SemanticValidationCase::EqualBarScaleBounds,
        Some("viz.bar_scale: min must be less than max")
    )]
    #[case::display_name_must_reference_a_value(
        SemanticValidationCase::UnknownDisplayName,
        Some("display.fictional-unknown-value: name must reference a query or formula")
    )]
    fn validates_widget_semantics(
        #[case] case: SemanticValidationCase,
        #[case] expected_error: Option<&str>,
    ) {
        let actual = semantic_validation_input(case)
            .validate()
            .map_err(|errors| errors.to_string());
        assert_eq!(
            actual,
            expected_error.map_or(Ok(()), |error| Err(error.to_string()))
        );
    }

    #[test]
    fn parses_reserved_metrics_and_visualizations_before_reporting_them_unsupported() {
        let parsed = WidgetDefinition::from_json(
            r#"{"queries":{"value":{"metric":"close","by":"stock"}},"viz":{"type":"line","series":["value"]}}"#,
        )
        .expect("parse reserved schema");

        let actual = parsed.validate().map_err(|errors| errors.to_string());
        assert_eq!(
            actual,
            Err("queries.value.metric: metric `close` is not supported yet; viz.type: line visualization is not supported yet".to_string()),
        );
    }

    #[rstest]
    #[case::trailing_operator(
        "sector +",
        "formulas.relative: invalid expression: expected a query name, number, or `(`"
    )]
    #[case::missing_parenthesis(
        "(sector + 1",
        "formulas.relative: invalid expression: expected `)`"
    )]
    #[case::unknown_character(
        "sector ^ 2",
        "formulas.relative: invalid expression: unexpected token at character 7"
    )]
    fn rejects_invalid_formulas_through_widget_validation(
        #[case] formula: &str,
        #[case] expected: &str,
    ) {
        let mut widget = definition();
        widget
            .formulas
            .insert("relative".to_string(), formula.to_string());

        assert_eq!(
            widget.validate().map_err(|errors| errors.to_string()),
            Err(expected.to_string()),
        );
    }

    #[derive(Clone, Copy)]
    enum FormulaLimit {
        Length,
        Nesting,
        Operators,
    }

    #[rstest]
    #[case::length(
        FormulaLimit::Length,
        "formulas.relative: invalid expression: formula must not exceed 1024 bytes"
    )]
    #[case::nesting(
        FormulaLimit::Nesting,
        "formulas.relative: invalid expression: formula nesting must not exceed 64 levels"
    )]
    #[case::operators(
        FormulaLimit::Operators,
        "formulas.relative: invalid expression: formula may contain at most 64 arithmetic operators"
    )]
    fn rejects_formulas_above_complexity_limits(
        #[case] limit: FormulaLimit,
        #[case] expected: &str,
    ) {
        let formula = match limit {
            FormulaLimit::Length => "x".repeat(1025),
            FormulaLimit::Nesting => {
                format!("{}sector{}", "(".repeat(65), ")".repeat(65))
            }
            FormulaLimit::Operators => format!("{}sector", "sector+".repeat(65)),
        };
        let mut widget = definition();
        widget.formulas.insert("relative".to_string(), formula);

        assert_eq!(
            widget.validate().map_err(|errors| errors.to_string()),
            Err(expected.to_string()),
        );
    }

    #[test]
    fn serializes_widget_definition_as_json_without_losing_optional_fields() {
        let actual = serde_json::to_value(definition()).expect("serialize definition");
        assert_eq!(
            actual,
            json!({
                "queries": {
                    "sector": {"metric": "change", "period": "1d", "by": "group", "axis": "demo-axis", "indicator": null, "universe": null, "agg": "mkt_cap_weighted"},
                    "market": {"metric": "change", "period": "1d", "by": "none", "axis": null, "indicator": null, "universe": null, "agg": "mkt_cap_weighted"}
                },
                "formulas": {"relative": "sector - market"},
                "viz": {"type": "treemap", "size": "market", "color": "relative"},
                "time": {"as_of": "2098-01-05", "range": null}
            }),
        );
    }
}
