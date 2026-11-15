use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use core_domain::document_search::PreparedDocumentSearch;
use thiserror::Error;

use crate::persistence::PersistenceError;

use super::types::DocumentSearchResult;

#[derive(Debug, Error)]
pub enum DocumentSearchRepositoryError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}

#[async_trait]
pub trait DocumentSearchRepository: Send + Sync {
    async fn search(
        &self,
        query: PreparedDocumentSearch,
        as_of: Option<DateTime<FixedOffset>>,
        stock_id: Option<String>,
        limit: u32,
    ) -> Result<Vec<DocumentSearchResult>, DocumentSearchRepositoryError>;
}

pub type SharedDocumentSearchRepository =
    std::sync::Arc<dyn DocumentSearchRepository + Send + Sync>;
