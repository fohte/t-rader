use async_trait::async_trait;

use crate::ingest_status::IngestStatusData;
use crate::persistence::PersistenceError;

#[async_trait]
pub trait IngestStatusRepository: Send + Sync {
    async fn read(&self) -> Result<IngestStatusData, PersistenceError>;
}

pub type SharedIngestStatusRepository = std::sync::Arc<dyn IngestStatusRepository + Send + Sync>;
