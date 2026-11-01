use chrono::{Datelike, Days, NaiveDate};
use core_application::widget::{
    WidgetAggregation, WidgetBy, WidgetDefinition, WidgetMetric, WidgetPeriod, WidgetQuery,
    WidgetTime, WidgetUniverse, WidgetVisualization, WidgetVisualizationOrder,
};

const EXPECTED_VALID_COMBINATIONS: usize = 129;
const EXPECTED_CASE_COUNT: usize = 296;
const GROUPS_PER_AXIS: usize = 30;
const STOCK_IDS: [&str; 8] = [
    "X001", "X002", "X003", "X004", "X005", "X006", "X007", "X008",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UniverseKind {
    All,
    Stocks,
    Group,
}

#[derive(Clone, Debug)]
pub(super) struct WorkloadCase {
    pub(super) id: String,
    pub(super) definition: WidgetDefinition,
    pub(super) expected_candidate_count: usize,
    pub(super) expected_row_count: usize,
    pub(super) expected_as_of: NaiveDate,
    pub(super) full_candidate_response: bool,
}

pub(super) fn build_cases(reference_date: NaiveDate) -> Result<Vec<WorkloadCase>, String> {
    let mut valid_combinations = Vec::new();
    for metric in [
        WidgetMetric::Change,
        WidgetMetric::Close,
        WidgetMetric::Turnover,
        WidgetMetric::MktCap,
        WidgetMetric::Per,
        WidgetMetric::Pbr,
        WidgetMetric::Indicator,
    ] {
        for by in [WidgetBy::None, WidgetBy::Stock, WidgetBy::Group] {
            for universe in [UniverseKind::All, UniverseKind::Stocks, UniverseKind::Group] {
                for period in [
                    None,
                    Some(WidgetPeriod::OneDay),
                    Some(WidgetPeriod::FiveDays),
                    Some(WidgetPeriod::TwentyDays),
                    Some(WidgetPeriod::SixtyDays),
                ] {
                    for aggregation in [
                        None,
                        Some(WidgetAggregation::MarketCapWeighted),
                        Some(WidgetAggregation::Mean),
                        Some(WidgetAggregation::Sum),
                        Some(WidgetAggregation::Count),
                    ] {
                        verify_period_variant(period);
                        let query =
                            make_query(metric, by, universe, period, aggregation, "demo-axis");
                        if definition(query.clone(), None).validate().is_ok() {
                            valid_combinations.push((query, universe));
                        }
                    }
                }
            }
        }
    }
    valid_combinations.retain(|(query, universe)| is_selected_combination(query, *universe));
    if valid_combinations.len() != EXPECTED_VALID_COMBINATIONS {
        return Err(format!(
            "widget enum matrix changed: expected {EXPECTED_VALID_COMBINATIONS} selected valid combinations, got {}",
            valid_combinations.len(),
        ));
    }

    let past_request = reference_date
        .checked_sub_days(Days::new(14))
        .ok_or_else(|| "reference date is outside the supported range".to_string())?;
    let past_as_of = previous_weekday(past_request);
    let mut cases = Vec::with_capacity(EXPECTED_CASE_COUNT);
    for (query, universe) in valid_combinations {
        for (requested, expected_as_of) in
            [(None, reference_date), (Some(past_request), past_as_of)]
        {
            let mut axes = vec![query.axis.clone()];
            if query.by == WidgetBy::Group && universe == UniverseKind::Group {
                axes.push(Some("sample-axis".to_string()));
            }
            for axis in axes {
                let mut query = query.clone();
                query.axis = axis;
                let case_index = cases.len();
                let full_candidate_response = is_full_candidate_response(&query);
                let full_group_response = query.by == WidgetBy::Group
                    && query.metric == WidgetMetric::Close
                    && query.agg == Some(WidgetAggregation::Mean)
                    && universe == UniverseKind::All;
                let mut definition = definition(query.clone(), requested);
                let expected_row_count = if full_candidate_response {
                    candidate_count(universe)
                } else if full_group_response {
                    GROUPS_PER_AXIS
                } else {
                    row_count(&query, universe)
                };
                if full_candidate_response || full_group_response {
                    definition.viz = WidgetVisualization::Table {
                        columns: vec!["value".to_string()],
                    };
                }
                cases.push(WorkloadCase {
                    id: format!("widget-{case_index:03}"),
                    definition,
                    expected_candidate_count: candidate_count(universe),
                    expected_row_count,
                    expected_as_of,
                    full_candidate_response,
                });
            }
        }
    }
    if cases.len() != EXPECTED_CASE_COUNT {
        return Err(format!(
            "widget workload matrix changed: expected {EXPECTED_CASE_COUNT} cases, got {}",
            cases.len()
        ));
    }
    Ok(cases)
}

pub(super) fn required_case_ids(cases: &[WorkloadCase]) -> Vec<String> {
    let mut ids = cases.iter().map(|case| case.id.clone()).collect::<Vec<_>>();
    ids.extend([
        "group-movers-short".to_string(),
        "group-movers-long".to_string(),
        "group-movers-short-filtered".to_string(),
        "group-movers-long-filtered".to_string(),
    ]);
    ids
}

pub(super) fn reference_date_definition() -> WidgetDefinition {
    definition(
        make_query(
            WidgetMetric::Close,
            WidgetBy::None,
            UniverseKind::All,
            None,
            Some(WidgetAggregation::Mean),
            "demo-axis",
        ),
        None,
    )
}

fn make_query(
    metric: WidgetMetric,
    by: WidgetBy,
    universe: UniverseKind,
    period: Option<WidgetPeriod>,
    aggregation: Option<WidgetAggregation>,
    axis: &str,
) -> WidgetQuery {
    let is_indicator = metric == WidgetMetric::Indicator;
    WidgetQuery {
        metric,
        period,
        by,
        axis: (by == WidgetBy::Group).then(|| axis.to_string()),
        indicator: is_indicator.then(|| "demo-indicator".to_string()),
        universe: match universe {
            UniverseKind::All => None,
            UniverseKind::Stocks => Some(WidgetUniverse::Stocks {
                stocks: STOCK_IDS.map(str::to_string).to_vec(),
            }),
            UniverseKind::Group => Some(WidgetUniverse::Group {
                group: "demo-axis/group-alpha".to_string(),
            }),
        },
        agg: aggregation,
    }
}

fn definition(query: WidgetQuery, as_of: Option<NaiveDate>) -> WidgetDefinition {
    WidgetDefinition {
        queries: std::collections::BTreeMap::from([("value".to_string(), query)]),
        formulas: std::collections::BTreeMap::new(),
        display: std::collections::BTreeMap::new(),
        viz: WidgetVisualization::Toplist {
            value: "value".to_string(),
            order: WidgetVisualizationOrder::Desc,
            limit: 5,
            bar_scale: None,
        },
        time: WidgetTime { as_of, range: None },
    }
}

fn candidate_count(universe: UniverseKind) -> usize {
    match universe {
        UniverseKind::All => 15_000,
        UniverseKind::Stocks => STOCK_IDS.len(),
        UniverseKind::Group => 500,
    }
}

pub(super) fn missing_target_group_definition(as_of: NaiveDate) -> WidgetDefinition {
    definition(
        WidgetQuery {
            metric: WidgetMetric::Close,
            period: None,
            by: WidgetBy::Stock,
            axis: None,
            indicator: None,
            universe: Some(WidgetUniverse::Group {
                group: "demo-axis/missing-group".to_string(),
            }),
            agg: None,
        },
        Some(as_of),
    )
}

fn is_full_candidate_response(query: &WidgetQuery) -> bool {
    query.metric == WidgetMetric::Close
        && query.by == WidgetBy::Stock
        && query.period.is_none()
        && query.agg.is_none()
}

fn is_selected_combination(query: &WidgetQuery, universe: UniverseKind) -> bool {
    match query.metric {
        WidgetMetric::Change => match (query.by, universe) {
            (WidgetBy::None, UniverseKind::All | UniverseKind::Group)
            | (WidgetBy::Stock, UniverseKind::All | UniverseKind::Group)
            | (WidgetBy::Group, UniverseKind::All | UniverseKind::Group) => true,
            (WidgetBy::Stock, UniverseKind::Stocks) => {
                query.period == Some(WidgetPeriod::OneDay) && is_unaggregated(query.agg)
            }
            (WidgetBy::None, UniverseKind::Stocks) | (WidgetBy::Group, UniverseKind::Stocks) => {
                false
            }
        },
        WidgetMetric::Indicator => true,
        WidgetMetric::Close => is_selected_regular_combination(query),
        WidgetMetric::Turnover => is_selected_regular_combination(query),
        WidgetMetric::MktCap => is_selected_regular_combination(query),
        WidgetMetric::Per => is_selected_regular_combination(query),
        WidgetMetric::Pbr => is_selected_regular_combination(query),
    }
}

fn is_selected_regular_combination(query: &WidgetQuery) -> bool {
    let metric = query.metric;
    match (query.by, query_universe_kind(query), query.agg) {
        (WidgetBy::None, UniverseKind::All, Some(WidgetAggregation::Mean))
        | (WidgetBy::None, UniverseKind::All, Some(WidgetAggregation::Sum))
        | (WidgetBy::None, UniverseKind::All, Some(WidgetAggregation::Count)) => true,
        (WidgetBy::None, UniverseKind::All, None)
        | (WidgetBy::None, UniverseKind::All, Some(WidgetAggregation::MarketCapWeighted)) => false,
        (WidgetBy::None, UniverseKind::Stocks, Some(WidgetAggregation::Mean)) => true,
        (WidgetBy::None, UniverseKind::Stocks, None)
        | (WidgetBy::None, UniverseKind::Stocks, Some(WidgetAggregation::MarketCapWeighted))
        | (WidgetBy::None, UniverseKind::Stocks, Some(WidgetAggregation::Sum))
        | (WidgetBy::None, UniverseKind::Stocks, Some(WidgetAggregation::Count)) => false,
        (WidgetBy::None, UniverseKind::Group, Some(WidgetAggregation::Mean)) => {
            matches!(metric, WidgetMetric::Per | WidgetMetric::Pbr)
        }
        (WidgetBy::None, UniverseKind::Group, None)
        | (WidgetBy::None, UniverseKind::Group, Some(WidgetAggregation::MarketCapWeighted))
        | (WidgetBy::None, UniverseKind::Group, Some(WidgetAggregation::Sum))
        | (WidgetBy::None, UniverseKind::Group, Some(WidgetAggregation::Count)) => false,
        (WidgetBy::Stock, UniverseKind::All, None)
        | (WidgetBy::Stock, UniverseKind::Stocks, None)
        | (WidgetBy::Stock, UniverseKind::Group, None) => true,
        (WidgetBy::Stock, UniverseKind::All, Some(WidgetAggregation::MarketCapWeighted))
        | (WidgetBy::Stock, UniverseKind::All, Some(WidgetAggregation::Mean))
        | (WidgetBy::Stock, UniverseKind::All, Some(WidgetAggregation::Sum))
        | (WidgetBy::Stock, UniverseKind::All, Some(WidgetAggregation::Count))
        | (WidgetBy::Stock, UniverseKind::Stocks, Some(WidgetAggregation::MarketCapWeighted))
        | (WidgetBy::Stock, UniverseKind::Stocks, Some(WidgetAggregation::Mean))
        | (WidgetBy::Stock, UniverseKind::Stocks, Some(WidgetAggregation::Sum))
        | (WidgetBy::Stock, UniverseKind::Stocks, Some(WidgetAggregation::Count))
        | (WidgetBy::Stock, UniverseKind::Group, Some(WidgetAggregation::MarketCapWeighted))
        | (WidgetBy::Stock, UniverseKind::Group, Some(WidgetAggregation::Mean))
        | (WidgetBy::Stock, UniverseKind::Group, Some(WidgetAggregation::Sum))
        | (WidgetBy::Stock, UniverseKind::Group, Some(WidgetAggregation::Count)) => false,
        (WidgetBy::Group, UniverseKind::All, Some(WidgetAggregation::Mean))
        | (WidgetBy::Group, UniverseKind::All, Some(WidgetAggregation::Sum))
        | (WidgetBy::Group, UniverseKind::All, Some(WidgetAggregation::Count)) => true,
        (WidgetBy::Group, UniverseKind::All, None)
        | (WidgetBy::Group, UniverseKind::All, Some(WidgetAggregation::MarketCapWeighted))
        | (WidgetBy::Group, UniverseKind::Stocks, None)
        | (WidgetBy::Group, UniverseKind::Stocks, Some(WidgetAggregation::MarketCapWeighted))
        | (WidgetBy::Group, UniverseKind::Stocks, Some(WidgetAggregation::Mean))
        | (WidgetBy::Group, UniverseKind::Stocks, Some(WidgetAggregation::Sum))
        | (WidgetBy::Group, UniverseKind::Stocks, Some(WidgetAggregation::Count)) => false,
        (WidgetBy::Group, UniverseKind::Group, Some(WidgetAggregation::Mean)) => matches!(
            metric,
            WidgetMetric::Close | WidgetMetric::Turnover | WidgetMetric::MktCap
        ),
        (WidgetBy::Group, UniverseKind::Group, None)
        | (WidgetBy::Group, UniverseKind::Group, Some(WidgetAggregation::MarketCapWeighted))
        | (WidgetBy::Group, UniverseKind::Group, Some(WidgetAggregation::Sum))
        | (WidgetBy::Group, UniverseKind::Group, Some(WidgetAggregation::Count)) => false,
    }
}

fn query_universe_kind(query: &WidgetQuery) -> UniverseKind {
    match query.universe.as_ref() {
        None => UniverseKind::All,
        Some(WidgetUniverse::Stocks { .. }) => UniverseKind::Stocks,
        Some(WidgetUniverse::Group { .. }) => UniverseKind::Group,
    }
}

fn is_unaggregated(aggregation: Option<WidgetAggregation>) -> bool {
    match aggregation {
        None => true,
        Some(WidgetAggregation::MarketCapWeighted)
        | Some(WidgetAggregation::Mean)
        | Some(WidgetAggregation::Sum)
        | Some(WidgetAggregation::Count) => false,
    }
}

fn verify_period_variant(period: Option<WidgetPeriod>) {
    match period {
        None
        | Some(WidgetPeriod::OneDay)
        | Some(WidgetPeriod::FiveDays)
        | Some(WidgetPeriod::TwentyDays)
        | Some(WidgetPeriod::SixtyDays) => {}
    }
}

fn row_count(query: &WidgetQuery, universe: UniverseKind) -> usize {
    match query.by {
        WidgetBy::None => 1,
        WidgetBy::Stock => 5,
        WidgetBy::Group => match query.metric {
            WidgetMetric::Change
            | WidgetMetric::Close
            | WidgetMetric::Turnover
            | WidgetMetric::MktCap => match universe {
                UniverseKind::All | UniverseKind::Stocks => 5,
                UniverseKind::Group => group_universe_row_count(query),
            },
            WidgetMetric::Per | WidgetMetric::Pbr => match universe {
                UniverseKind::All | UniverseKind::Stocks => 1,
                UniverseKind::Group => group_universe_row_count(query),
            },
            WidgetMetric::Indicator => 1,
        },
    }
}

fn group_universe_row_count(query: &WidgetQuery) -> usize {
    let output_axis = query.axis.as_deref();
    let universe_axis = query
        .universe
        .as_ref()
        .and_then(WidgetUniverse::group_parts)
        .map(|(axis, _)| axis);
    if output_axis == universe_axis { 1 } else { 5 }
}

fn previous_weekday(mut date: NaiveDate) -> NaiveDate {
    while matches!(date.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun) {
        date -= chrono::Duration::days(1);
    }
    date
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use chrono::NaiveDate;
    use core_application::widget::{WidgetAggregation, WidgetBy, WidgetMetric, WidgetUniverse};

    use super::{build_cases, required_case_ids};

    #[test]
    fn enumerates_the_required_widget_case_set() {
        let reference_date = NaiveDate::from_ymd_opt(2099, 6, 30);
        let actual = reference_date
            .map(build_cases)
            .transpose()
            .map(|cases| cases.map(|cases| required_case_ids(&cases)));
        let expected = (0..296)
            .map(|index| format!("widget-{index:03}"))
            .chain([
                "group-movers-short".to_string(),
                "group-movers-long".to_string(),
                "group-movers-short-filtered".to_string(),
                "group-movers-long-filtered".to_string(),
            ])
            .collect::<Vec<_>>();
        assert_eq!(actual, Ok(Some(expected)));
    }

    #[test]
    fn checks_candidate_counts_and_group_axis_directions() {
        let reference_date = NaiveDate::from_ymd_opt(2099, 6, 30);
        let actual = reference_date.map(build_cases).transpose().map(|cases| {
            cases.map(|cases| {
                let candidate_counts = cases
                    .iter()
                    .filter(|case| case.full_candidate_response)
                    .map(|case| case.expected_candidate_count)
                    .collect::<BTreeSet<_>>();
                let group_axis_rows = cases
                    .iter()
                    .filter_map(|case| {
                        let query = case.definition.queries.get("value")?;
                        let has_group_universe =
                            matches!(query.universe.as_ref(), Some(WidgetUniverse::Group { .. }));
                        (query.metric == WidgetMetric::Close
                            && query.by == WidgetBy::Group
                            && query.period.is_none()
                            && query.agg == Some(WidgetAggregation::Mean)
                            && has_group_universe
                            && case.definition.time.as_of.is_none())
                        .then(|| (query.axis.clone(), case.expected_row_count))
                    })
                    .collect::<Vec<_>>();
                let sparse_group_rows = cases
                    .iter()
                    .filter_map(|case| {
                        let query = case.definition.queries.get("value")?;
                        (matches!(query.metric, WidgetMetric::Per | WidgetMetric::Pbr)
                            && query.by == WidgetBy::Group
                            && query.universe.is_none()
                            && query.period.is_none()
                            && query.agg == Some(WidgetAggregation::Mean)
                            && case.definition.time.as_of.is_none())
                        .then_some((query.metric, case.expected_row_count))
                    })
                    .collect::<Vec<_>>();
                (candidate_counts, group_axis_rows, sparse_group_rows)
            })
        });
        let expected = Ok(Some((
            BTreeSet::from([8, 500, 15_000]),
            vec![
                (Some("demo-axis".to_string()), 1),
                (Some("sample-axis".to_string()), 5),
            ],
            vec![(WidgetMetric::Per, 1), (WidgetMetric::Pbr, 1)],
        )));
        assert_eq!(actual, expected);
    }
}
