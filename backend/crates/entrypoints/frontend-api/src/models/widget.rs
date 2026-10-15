use std::collections::BTreeMap;

use chrono::NaiveDate;
use core_application::widget::{
    WidgetAggregation, WidgetBy, WidgetDefinition, WidgetMetric, WidgetPeriod, WidgetQuery,
    WidgetTime, WidgetUniverse, WidgetVisualization, WidgetVisualizationOrder,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WidgetQueryRequest {
    pub queries: BTreeMap<String, WidgetQueryRequestDefinition>,
    #[serde(default)]
    pub formulas: BTreeMap<String, String>,
    pub viz: WidgetVisualizationRequest,
    #[serde(default)]
    pub time: WidgetTimeRequest,
}

impl From<WidgetQueryRequest> for WidgetDefinition {
    fn from(request: WidgetQueryRequest) -> Self {
        Self {
            queries: request
                .queries
                .into_iter()
                .map(|(name, query)| (name, query.into()))
                .collect(),
            formulas: request.formulas,
            viz: request.viz.into(),
            time: request.time.into(),
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WidgetQueryRequestDefinition {
    pub metric: WidgetMetricRequest,
    #[serde(default)]
    pub period: Option<WidgetPeriodRequest>,
    pub by: WidgetByRequest,
    #[serde(default)]
    pub axis: Option<String>,
    #[serde(default)]
    pub indicator: Option<String>,
    #[serde(default)]
    pub universe: Option<WidgetUniverseRequest>,
    #[serde(default)]
    pub agg: Option<WidgetAggregationRequest>,
}

impl From<WidgetQueryRequestDefinition> for WidgetQuery {
    fn from(query: WidgetQueryRequestDefinition) -> Self {
        Self {
            metric: query.metric.into(),
            period: query.period.map(Into::into),
            by: query.by.into(),
            axis: query.axis,
            indicator: query.indicator,
            universe: query.universe.map(Into::into),
            agg: query.agg.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WidgetMetricRequest {
    Change,
    Close,
    Turnover,
    #[serde(rename = "mkt_cap")]
    MktCap,
    Per,
    Pbr,
    Indicator,
}

impl From<WidgetMetricRequest> for WidgetMetric {
    fn from(metric: WidgetMetricRequest) -> Self {
        match metric {
            WidgetMetricRequest::Change => Self::Change,
            WidgetMetricRequest::Close => Self::Close,
            WidgetMetricRequest::Turnover => Self::Turnover,
            WidgetMetricRequest::MktCap => Self::MktCap,
            WidgetMetricRequest::Per => Self::Per,
            WidgetMetricRequest::Pbr => Self::Pbr,
            WidgetMetricRequest::Indicator => Self::Indicator,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
pub enum WidgetPeriodRequest {
    #[serde(rename = "1d")]
    OneDay,
    #[serde(rename = "5d")]
    FiveDays,
    #[serde(rename = "20d")]
    TwentyDays,
    #[serde(rename = "60d")]
    SixtyDays,
}

impl From<WidgetPeriodRequest> for WidgetPeriod {
    fn from(period: WidgetPeriodRequest) -> Self {
        match period {
            WidgetPeriodRequest::OneDay => Self::OneDay,
            WidgetPeriodRequest::FiveDays => Self::FiveDays,
            WidgetPeriodRequest::TwentyDays => Self::TwentyDays,
            WidgetPeriodRequest::SixtyDays => Self::SixtyDays,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WidgetByRequest {
    None,
    Stock,
    Group,
}

impl From<WidgetByRequest> for WidgetBy {
    fn from(by: WidgetByRequest) -> Self {
        match by {
            WidgetByRequest::None => Self::None,
            WidgetByRequest::Stock => Self::Stock,
            WidgetByRequest::Group => Self::Group,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum WidgetUniverseRequest {
    Group { group: String },
    Stocks { stocks: Vec<String> },
}

impl From<WidgetUniverseRequest> for WidgetUniverse {
    fn from(universe: WidgetUniverseRequest) -> Self {
        match universe {
            WidgetUniverseRequest::Group { group } => Self::Group { group },
            WidgetUniverseRequest::Stocks { stocks } => Self::Stocks { stocks },
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WidgetAggregationRequest {
    #[serde(rename = "mkt_cap_weighted")]
    MarketCapWeighted,
    Mean,
    Sum,
    Count,
}

impl From<WidgetAggregationRequest> for WidgetAggregation {
    fn from(aggregation: WidgetAggregationRequest) -> Self {
        match aggregation {
            WidgetAggregationRequest::MarketCapWeighted => Self::MarketCapWeighted,
            WidgetAggregationRequest::Mean => Self::Mean,
            WidgetAggregationRequest::Sum => Self::Sum,
            WidgetAggregationRequest::Count => Self::Count,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum WidgetVisualizationRequest {
    Treemap {
        size: String,
        color: String,
    },
    Toplist {
        value: String,
        order: WidgetVisualizationOrderRequest,
        limit: u32,
    },
    Table {
        columns: Vec<String>,
    },
    Line {
        series: Vec<String>,
    },
}

impl From<WidgetVisualizationRequest> for WidgetVisualization {
    fn from(viz: WidgetVisualizationRequest) -> Self {
        match viz {
            WidgetVisualizationRequest::Treemap { size, color } => Self::Treemap { size, color },
            WidgetVisualizationRequest::Toplist {
                value,
                order,
                limit,
            } => Self::Toplist {
                value,
                order: order.into(),
                limit,
            },
            WidgetVisualizationRequest::Table { columns } => Self::Table { columns },
            WidgetVisualizationRequest::Line { series } => Self::Line { series },
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WidgetVisualizationOrderRequest {
    Asc,
    Desc,
}

impl From<WidgetVisualizationOrderRequest> for WidgetVisualizationOrder {
    fn from(order: WidgetVisualizationOrderRequest) -> Self {
        match order {
            WidgetVisualizationOrderRequest::Asc => Self::Asc,
            WidgetVisualizationOrderRequest::Desc => Self::Desc,
        }
    }
}

#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WidgetTimeRequest {
    pub as_of: Option<NaiveDate>,
    pub range: Option<String>,
}

impl From<WidgetTimeRequest> for WidgetTime {
    fn from(time: WidgetTimeRequest) -> Self {
        Self {
            as_of: time.as_of,
            range: time.range,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WidgetQueryResponse {
    pub as_of: NaiveDate,
    pub rows: Vec<WidgetQueryResultRow>,
}

impl From<core_application::widget::WidgetQueryResponse> for WidgetQueryResponse {
    fn from(response: core_application::widget::WidgetQueryResponse) -> Self {
        Self {
            as_of: response.as_of,
            rows: response
                .rows
                .into_iter()
                .map(WidgetQueryResultRow::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WidgetQueryResultRow {
    pub key: String,
    pub name: String,
    pub values: BTreeMap<String, Option<Decimal>>,
}

impl From<core_application::widget::WidgetValueRow> for WidgetQueryResultRow {
    fn from(row: core_application::widget::WidgetValueRow) -> Self {
        Self {
            key: row.key.as_string(),
            name: row.name,
            values: row
                .values
                .into_iter()
                .map(|(name, value)| (name, value.map(|value| value.normalize())))
                .collect(),
        }
    }
}
