use chrono::NaiveDate;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortRatioQuery {
    pub sector33_code: String,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub limit: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct ShortRatioIngestStats {
    pub days_fetched: usize,
    pub rows_upserted: usize,
}
