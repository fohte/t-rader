use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndicatorObservation {
    pub date: NaiveDate,
    pub value: Decimal,
}

#[derive(Debug, thiserror::Error)]
pub enum IndicatorObservationSourceError {
    #[error("client initialization error: {0}")]
    Initialization(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("api error (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("failed to parse response: {0}")]
    Parse(String),
}

#[async_trait]
pub trait IndicatorObservationSource: Send + Sync {
    async fn fetch_observations(
        &self,
        series_id: &str,
        observation_start: Option<NaiveDate>,
    ) -> Result<Vec<IndicatorObservation>, IndicatorObservationSourceError>;
}

pub type SharedIndicatorObservationSource = Arc<dyn IndicatorObservationSource>;
