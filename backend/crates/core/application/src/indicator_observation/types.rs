use chrono::NaiveDate;
use core_domain::IndicatorObservation;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndicatorObservationMetadata {
    pub indicator_id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndicatorObservationSeriesDefinition {
    pub series_id: &'static str,
    pub indicator_id: &'static str,
    pub name: &'static str,
    pub kind: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndicatorObservationQuery {
    pub indicator_id: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndicatorObservationReadResult {
    pub indicator_id: String,
    pub observations: Vec<IndicatorObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndicatorObservationIngestResult {
    pub series: Vec<IndicatorObservationIngestSeriesResult>,
    pub errors: Vec<IndicatorObservationBatchError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndicatorObservationBatchError {
    pub date: NaiveDate,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum IndicatorObservationIngestSeriesResult {
    Succeeded { series_id: String, upserted: usize },
    Failed { series_id: String, error: String },
}
