use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::IndicatorObservation;

use crate::indicator_observation_source::IndicatorObservationSourceError;

#[async_trait]
pub trait IndicatorObservationBatchSource: Send + Sync {
    async fn fetch_observations(
        &self,
        series_ids: &[&str],
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<HashMap<String, Vec<IndicatorObservation>>, IndicatorObservationSourceError>;
}

pub type SharedIndicatorObservationBatchSource =
    Arc<dyn IndicatorObservationBatchSource + Send + Sync>;
