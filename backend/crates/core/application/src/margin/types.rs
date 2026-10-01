use chrono::NaiveDate;
use core_domain::margin::{MarginAlertRecord, MarginInterestRecord};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarginQuery {
    pub symbol: String,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub limit: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarginReadResult {
    pub interest: Vec<MarginInterestRecord>,
    pub alerts: Vec<MarginAlertRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct IngestStats {
    pub days_fetched: usize,
    pub rows_upserted: usize,
}
