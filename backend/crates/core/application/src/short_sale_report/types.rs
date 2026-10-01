use chrono::NaiveDate;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortSaleReportQuery {
    pub code_from: String,
    pub code_to: String,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub limit: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct ShortSaleReportIngestStats {
    pub days_fetched: usize,
    pub rows_upserted: usize,
}
