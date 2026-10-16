use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidgetDefinition {
    pub queries: BTreeMap<String, WidgetQuery>,
    #[serde(default)]
    pub formulas: BTreeMap<String, String>,
    pub viz: WidgetVisualization,
    #[serde(default)]
    pub time: WidgetTime,
}

impl WidgetDefinition {
    pub fn from_json(input: &str) -> Result<Self, WidgetDefinitionParseError> {
        serde_json::from_str(input).map_err(WidgetDefinitionParseError::Json)
    }

    pub fn from_yaml(input: &str) -> Result<Self, WidgetDefinitionParseError> {
        serde_yaml_ng::from_str(input).map_err(WidgetDefinitionParseError::Yaml)
    }
}

#[derive(Debug, Error)]
pub enum WidgetDefinitionParseError {
    #[error("invalid JSON widget definition: {0}")]
    Json(#[source] serde_json::Error),
    #[error("invalid YAML widget definition: {0}")]
    Yaml(#[source] serde_yaml_ng::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidgetQuery {
    pub metric: WidgetMetric,
    #[serde(default)]
    pub period: Option<WidgetPeriod>,
    pub by: WidgetBy,
    #[serde(default)]
    pub axis: Option<String>,
    #[serde(default)]
    pub indicator: Option<String>,
    #[serde(default)]
    pub universe: Option<WidgetUniverse>,
    #[serde(default)]
    pub agg: Option<WidgetAggregation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetMetric {
    Change,
    Close,
    Turnover,
    #[serde(rename = "mkt_cap")]
    MktCap,
    Per,
    Pbr,
    Indicator,
}

impl WidgetMetric {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Change => "change",
            Self::Close => "close",
            Self::Turnover => "turnover",
            Self::MktCap => "mkt_cap",
            Self::Per => "per",
            Self::Pbr => "pbr",
            Self::Indicator => "indicator",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetPeriod {
    #[serde(rename = "1d")]
    OneDay,
    #[serde(rename = "5d")]
    FiveDays,
    #[serde(rename = "20d")]
    TwentyDays,
    #[serde(rename = "60d")]
    SixtyDays,
}

impl WidgetPeriod {
    pub fn trading_days(self) -> i64 {
        match self {
            Self::OneDay => 1,
            Self::FiveDays => 5,
            Self::TwentyDays => 20,
            Self::SixtyDays => 60,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetBy {
    None,
    Stock,
    Group,
}

impl WidgetBy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Stock => "stock",
            Self::Group => "group",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum WidgetUniverse {
    Group { group: String },
    Stocks { stocks: Vec<String> },
}

impl WidgetUniverse {
    pub fn group_parts(&self) -> Option<(&str, &str)> {
        let Self::Group { group } = self else {
            return None;
        };
        let (axis, key) = group.split_once('/')?;
        Some((axis, key))
    }

    pub fn stock_ids(&self) -> Option<&[String]> {
        match self {
            Self::Group { .. } => None,
            Self::Stocks { stocks } => Some(stocks),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetAggregation {
    #[serde(rename = "mkt_cap_weighted")]
    MarketCapWeighted,
    Mean,
    Sum,
    Count,
}

impl WidgetAggregation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MarketCapWeighted => "mkt_cap_weighted",
            Self::Mean => "mean",
            Self::Sum => "sum",
            Self::Count => "count",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum WidgetVisualization {
    Treemap {
        size: String,
        color: String,
    },
    Toplist {
        value: String,
        order: WidgetVisualizationOrder,
        limit: u32,
    },
    Table {
        columns: Vec<String>,
    },
    Line {
        series: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetVisualizationOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidgetTime {
    #[serde(default)]
    pub as_of: Option<NaiveDate>,
    #[serde(default)]
    pub range: Option<String>,
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::rstest;

    use super::WidgetDefinition;

    #[rstest]
    #[case::group(indoc! {r#"
        {
          "queries": {
            "sample": {
              "metric": "change",
              "period": "1d",
              "by": "group",
              "axis": "demo-axis",
              "universe": {
                "group": "demo-axis/demo-group",
                "unexpected": "value"
              },
              "agg": "mean"
            }
          },
          "viz": {
            "type": "treemap",
            "size": "sample",
            "color": "sample"
          }
        }
    "#})]
    #[case::stocks(indoc! {r#"
        {
          "queries": {
            "sample": {
              "metric": "change",
              "period": "1d",
              "by": "stock",
              "universe": {
                "stocks": ["demo-code"],
                "unexpected": "value"
              },
              "agg": "mean"
            }
          },
          "viz": {
            "type": "treemap",
            "size": "sample",
            "color": "sample"
          }
        }
    "#})]
    fn json_rejects_unknown_universe_fields(#[case] input: &str) {
        assert_eq!(
            WidgetDefinition::from_json(input)
                .map(|_| ())
                .map_err(|_| ()),
            Err(())
        );
    }

    #[rstest]
    #[case::group(indoc! {"
        queries:
          sample:
            metric: change
            period: 1d
            by: group
            axis: demo-axis
            universe:
              group: demo-axis/demo-group
              unexpected: value
            agg: mean
        viz:
          type: treemap
          size: sample
          color: sample
    "})]
    #[case::stocks(indoc! {"
        queries:
          sample:
            metric: change
            period: 1d
            by: stock
            universe:
              stocks:
                - demo-code
              unexpected: value
            agg: mean
        viz:
          type: treemap
          size: sample
          color: sample
    "})]
    fn yaml_rejects_unknown_universe_fields(#[case] input: &str) {
        assert_eq!(
            WidgetDefinition::from_yaml(input)
                .map(|_| ())
                .map_err(|_| ()),
            Err(())
        );
    }
}
