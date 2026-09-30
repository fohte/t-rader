use async_trait::async_trait;
use thiserror::Error;

use crate::persistence::PersistenceError;
use core_domain::short_ratio::ShortRatio;

use super::types::ShortRatioQuery;

#[derive(Debug, Error)]
pub enum ShortRatioRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait ShortRatioRepository: Send + Sync {
    async fn find_latest_date(
        &self,
    ) -> Result<Option<chrono::NaiveDate>, ShortRatioRepositoryError>;
    async fn upsert(&self, ratios: Vec<ShortRatio>) -> Result<(), ShortRatioRepositoryError>;
    async fn read(
        &self,
        query: ShortRatioQuery,
    ) -> Result<Vec<ShortRatio>, ShortRatioRepositoryError>;
}

pub type SharedShortRatioRepository = std::sync::Arc<dyn ShortRatioRepository + Send + Sync>;
