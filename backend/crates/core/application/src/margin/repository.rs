use async_trait::async_trait;
use thiserror::Error;

use crate::margin::types::{MarginQuery, MarginReadResult};
use crate::persistence::PersistenceError;
use core_domain::margin::{MarginAlertRecord, MarginInterestRecord};

#[derive(Debug, Error)]
pub enum MarginRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait MarginRepository: Send + Sync {
    async fn upsert_margin_interest(
        &self,
        records: Vec<MarginInterestRecord>,
    ) -> Result<(), MarginRepositoryError>;

    async fn find_latest_margin_interest_date(
        &self,
    ) -> Result<Option<chrono::NaiveDate>, MarginRepositoryError>;

    async fn upsert_margin_alert(
        &self,
        records: Vec<MarginAlertRecord>,
    ) -> Result<(), MarginRepositoryError>;

    async fn find_latest_margin_alert_pub_date(
        &self,
    ) -> Result<Option<chrono::NaiveDate>, MarginRepositoryError>;

    async fn read(&self, query: MarginQuery) -> Result<MarginReadResult, MarginRepositoryError>;
}

pub type SharedMarginRepository = std::sync::Arc<dyn MarginRepository>;
