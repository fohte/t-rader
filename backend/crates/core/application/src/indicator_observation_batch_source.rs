use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::IndicatorObservation;

use crate::indicator_observation::IndicatorObservationBatchError;
use crate::indicator_observation_source::IndicatorObservationSourceError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndicatorObservationBatch {
    pub observations: HashMap<String, Vec<IndicatorObservation>>,
    pub errors: Vec<IndicatorObservationBatchError>,
}

#[async_trait]
pub trait IndicatorObservationBatchSource: Send + Sync {
    async fn fetch_observations(
        &self,
        series_ids: &[&str],
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<IndicatorObservationBatch, IndicatorObservationSourceError>;
}

pub type SharedIndicatorObservationBatchSource =
    Arc<dyn IndicatorObservationBatchSource + Send + Sync>;
