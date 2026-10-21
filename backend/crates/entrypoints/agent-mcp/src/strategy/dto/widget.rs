use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryWidgetParams {
    /// ノートの `widget` fenced block 内に書く YAML。fence は含めない。
    pub definition_yaml: String,
}

#[cfg_attr(test, derive(Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct QueryWidgetResult {
    pub as_of: NaiveDate,
    pub rows: Vec<QueryWidgetRowDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<Vec<QueryWidgetSeriesDto>>,
}

#[cfg_attr(test, derive(Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct QueryWidgetSeriesDto {
    pub key: String,
    pub query: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<QueryWidgetSeriesTargetDto>,
    pub points: Vec<QueryWidgetSeriesPointDto>,
}

#[cfg_attr(test, derive(Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct QueryWidgetSeriesTargetDto {
    pub key: String,
    pub name: String,
}

#[cfg_attr(test, derive(Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct QueryWidgetSeriesPointDto {
    pub date: NaiveDate,
    #[schemars(with = "Option<f64>")]
    pub value: Option<rust_decimal::Decimal>,
}

#[cfg_attr(test, derive(Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct QueryWidgetRowDto {
    pub key: String,
    pub name: String,
    #[schemars(with = "std::collections::BTreeMap<String, Option<f64>>")]
    pub values: std::collections::BTreeMap<String, Option<rust_decimal::Decimal>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification: Option<QueryWidgetRowClassificationDto>,
}

#[cfg_attr(test, derive(Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct QueryWidgetRowClassificationDto {
    pub axis: String,
    pub group: Option<QueryWidgetGroupDto>,
}

#[cfg_attr(test, derive(Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct QueryWidgetGroupDto {
    pub key: String,
    pub name: String,
}
