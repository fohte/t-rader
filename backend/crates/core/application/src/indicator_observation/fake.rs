use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::IndicatorObservation;
use rust_decimal::Decimal;

use super::error::IndicatorObservationRepositoryError;
use super::repository::IndicatorObservationRepository;
use super::types::{IndicatorObservationMetadata, IndicatorObservationQuery};

#[derive(Default)]
pub struct FakeIndicatorObservationRepository {
    pub indicators: Mutex<HashMap<String, IndicatorObservationMetadata>>,
    pub observations: Mutex<BTreeMap<(String, NaiveDate), Decimal>>,
}

impl FakeIndicatorObservationRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn seed_observation(
        &self,
        indicator_id: impl Into<String>,
        date: NaiveDate,
        value: Decimal,
    ) {
        self.observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert((indicator_id.into(), date), value);
    }
}

#[async_trait]
impl IndicatorObservationRepository for FakeIndicatorObservationRepository {
    async fn ensure_indicator(
        &self,
        metadata: IndicatorObservationMetadata,
    ) -> Result<(), IndicatorObservationRepositoryError> {
        self.indicators
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(metadata.indicator_id.clone())
            .or_insert(metadata);
        Ok(())
    }

    async fn find_latest_date(
        &self,
        indicator_id: &str,
    ) -> Result<Option<NaiveDate>, IndicatorObservationRepositoryError> {
        Ok(self
            .observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keys()
            .filter(|(id, _)| id == indicator_id)
            .map(|(_, date)| *date)
            .max())
    }

    async fn upsert_observations(
        &self,
        indicator_id: &str,
        observations: Vec<IndicatorObservation>,
    ) -> Result<usize, IndicatorObservationRepositoryError> {
        let count = observations.len();
        let mut stored = self
            .observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for observation in observations {
            stored.insert(
                (indicator_id.to_string(), observation.date),
                observation.value,
            );
        }
        Ok(count)
    }

    async fn find_observations(
        &self,
        query: IndicatorObservationQuery,
    ) -> Result<Vec<IndicatorObservation>, IndicatorObservationRepositoryError> {
        Ok(self
            .observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|((indicator_id, date), _)| {
                indicator_id == &query.indicator_id && *date >= query.from && *date <= query.to
            })
            .map(|((_, date), value)| IndicatorObservation {
                date: *date,
                value: *value,
            })
            .collect())
    }
}
