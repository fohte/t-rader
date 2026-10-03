use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadShortSaleReportsParams {
    pub symbol: String,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub limit: Option<u32>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ShortSaleReportDto {
    pub disc_date: NaiveDate,
    pub calc_date: NaiveDate,
    pub reporter_name: String,
    pub reporter_address: Option<String>,
    pub client_name: Option<String>,
    pub client_address: Option<String>,
    pub fund_name: Option<String>,
    pub short_position_ratio: f64,
    pub short_position_shares: i64,
    pub short_position_units: i64,
    pub prev_report_date: Option<NaiveDate>,
    pub prev_report_ratio: Option<f64>,
    pub notes: Option<String>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadShortSaleReportsResult {
    pub symbol: String,
    pub items: Vec<ShortSaleReportDto>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadSectorShortRatioParams {
    pub sector: String,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub limit: Option<u32>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct SectorShortRatioDto {
    pub date: NaiveDate,
    pub sell_excluding_short_value: Option<f64>,
    pub short_with_restriction_value: Option<f64>,
    pub short_without_restriction_value: Option<f64>,
    pub short_ratio: Option<f64>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadSectorShortRatioResult {
    pub sector: String,
    pub items: Vec<SectorShortRatioDto>,
}
