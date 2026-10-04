use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use uuid::Uuid;

use crate::persistence::PersistenceError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingNewsContent {
    pub news_item_id: Uuid,
    pub url: String,
}

#[async_trait]
pub trait NewsContentRepository: Send + Sync {
    async fn expire_pending_before(
        &self,
        cutoff: DateTime<FixedOffset>,
    ) -> Result<u64, PersistenceError>;

    async fn list_pending(&self, limit: u64) -> Result<Vec<PendingNewsContent>, PersistenceError>;

    async fn mark_fetched(&self, news_item_id: Uuid, body: String) -> Result<(), PersistenceError>;

    async fn mark_failed(&self, news_item_id: Uuid, error: String) -> Result<(), PersistenceError>;
}

pub type SharedNewsContentRepository = std::sync::Arc<dyn NewsContentRepository + Send + Sync>;
