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
        WidgetVisualization::Treemap { size, color } => {
            validate_value_reference("viz.size", size, definition, &mut errors);
            validate_value_reference("viz.color", color, definition, &mut errors);
            validate_matching_dimensions(
                "viz",
                [size.as_str(), color.as_str()],
                definition,
                &parsed_formulas,
                &mut errors,
            );
        }
        WidgetVisualization::Toplist { value, limit, .. } => {
            validate_value_reference("viz.value", value, definition, &mut errors);
            if *limit == 0 {
                errors.push(WidgetValidationError::new(
                    "viz.limit",
                    "limit must be greater than zero",
                ));
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
            viz: WidgetVisualization::Treemap {
                size: "market".to_string(),
                color: "relative".to_string(),
            },
            time: WidgetTime {
                as_of: Some(NaiveDate::from_ymd_opt(2098, 1, 5).expect("valid date")),
                range: None,
            },
        }
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
            viz: WidgetVisualization::Toplist {
                value: "market".to_string(),
                order: WidgetVisualizationOrder::Desc,
                limit: 10,
            },
            time: WidgetTime::default(),
        };
        assert_eq!(parsed, expected);
    }

    #[rstest]
    #[case::valid(definition(), None)]
    #[case::unsupported_metric({ let mut value = definition(); let query = value.queries.get_mut("market").expect("query"); query.metric = WidgetMetric::Per; query.agg = Some(WidgetAggregation::Count); value }, Some("queries.market.metric: metric `per` is not supported yet"))]
    #[case::unsupported_range({ let mut value = definition(); value.time.range = Some("1y".to_string()); value }, Some("time.range: time ranges are not supported yet"))]
    #[case::unknown_viz_value({ let mut value = definition(); value.viz = WidgetVisualization::Treemap { size: "missing".to_string(), color: "relative".to_string() }; value }, Some("viz.size: unknown query or formula name: `missing`"))]
    fn validates_widget_semantics(
        #[case] widget: WidgetDefinition,
        #[case] expected_error: Option<&str>,
    ) {
        let actual = widget.validate().map_err(|errors| errors.to_string());
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
