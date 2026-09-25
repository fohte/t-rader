use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::IndicatorObservation;

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
    /// 観測値を指定日以降で取得する。`None` の場合は source のデフォルト開始日を使う。
    /// source が欠損値を返す場合、その値は結果に含めない。
    async fn fetch_observations(
        &self,
        series_id: &str,
        observation_start: Option<NaiveDate>,
    ) -> Result<Vec<IndicatorObservation>, IndicatorObservationSourceError>;
}

pub type SharedIndicatorObservationSource = Arc<dyn IndicatorObservationSource>;
