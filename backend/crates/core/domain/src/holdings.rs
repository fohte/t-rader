use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareholdingDocumentMetadata {
    pub document_id: String,
    pub stock_code: Option<String>,
    pub filer_code: String,
    pub submitted_on: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LargeVolumeShareholdingDocument {
    pub metadata: ShareholdingDocumentMetadata,
    pub content: LargeVolumeShareholdingContent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LargeVolumeShareholdingContent {
    pub report_type: LargeVolumeReportType,
    pub change_reason: Option<String>,
    pub total_shares_ratio: Option<f64>,
    pub previous_total_shares_ratio: Option<f64>,
    pub holders: Vec<LargeVolumeHolder>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LargeVolumeReportType {
    Report,
    Amendment,
    AmendmentRapidTransfer,
    ReportSpecial,
    AmendmentSpecial,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LargeVolumeHolder {
    pub name: String,
    pub holding_purpose: Option<String>,
    pub shares_held: Option<i64>,
    pub shares_ratio: Option<f64>,
    pub previous_shares_ratio: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MajorShareholderDocument {
    pub metadata: ShareholdingDocumentMetadata,
    pub content: MajorShareholderContent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MajorShareholderContent {
    pub period_end: Option<NaiveDate>,
    pub report_type: MajorShareholderReportType,
    pub holders: Vec<MajorShareholder>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MajorShareholderReportType {
    Annual,
    Quarterly,
    SemiAnnual,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MajorShareholder {
    pub rank: Option<i32>,
    pub name: String,
    pub shares_held: Option<i64>,
    pub shares_ratio: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossShareholdingDocument {
    pub metadata: ShareholdingDocumentMetadata,
    pub content: CrossShareholdingContent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossShareholdingContent {
    pub period_end: Option<NaiveDate>,
    pub holdings: Vec<CrossShareholding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossShareholdingCategory {
    Specified,
    Deemed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutualHolding {
    Held,
    NotHeld,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossShareholding {
    pub issuer_name: String,
    pub issuer_stock_code: Option<String>,
    pub category: CrossShareholdingCategory,
    pub current_shares: Option<i64>,
    pub previous_shares: Option<i64>,
    pub current_book_value: Option<i64>,
    pub previous_book_value: Option<i64>,
    pub mutual_holding: MutualHolding,
}
