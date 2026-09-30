use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::IndicatorObservation;

use super::error::IndicatorObservationRepositoryError;
use super::types::{IndicatorObservationMetadata, IndicatorObservationQuery};

#[async_trait]
pub trait IndicatorObservationRepository: Send + Sync {
    async fn ensure_indicator(
        &self,
        metadata: IndicatorObservationMetadata,
    ) -> Result<(), IndicatorObservationRepositoryError>;

    async fn find_latest_date(
        &self,
        indicator_id: &str,
    ) -> Result<Option<NaiveDate>, IndicatorObservationRepositoryError>;

    async fn upsert_observations(
        &self,
        indicator_id: &str,
        observations: Vec<IndicatorObservation>,
    ) -> Result<usize, IndicatorObservationRepositoryError>;

    async fn find_observations(
        &self,
        query: IndicatorObservationQuery,
    ) -> Result<Vec<IndicatorObservation>, IndicatorObservationRepositoryError>;
}

pub type SharedIndicatorObservationRepository =
    Arc<dyn IndicatorObservationRepository + Send + Sync>;
