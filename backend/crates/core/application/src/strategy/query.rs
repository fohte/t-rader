use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use crate::persistence::PersistenceError;

use super::types::StrategySummary;

#[derive(Debug, Error)]
pub enum StrategySummaryQueryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait StrategySummaryQuery: Send + Sync {
    async fn list(&self) -> Result<Vec<StrategySummary>, StrategySummaryQueryError>;
}

pub type SharedStrategySummaryQuery = Arc<dyn StrategySummaryQuery + Send + Sync>;
