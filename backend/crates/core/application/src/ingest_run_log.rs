use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use serde_json::Value;
use uuid::Uuid;

use crate::persistence::PersistenceError;

#[async_trait]
pub trait IngestRunLog: Send + Sync {
    async fn start(&self, job: &str) -> Result<Uuid, PersistenceError>;

    async fn finish(
        &self,
        run_id: Uuid,
        result: Result<Value, String>,
    ) -> Result<(), PersistenceError>;

    async fn finish_with_stats_and_error(
        &self,
        run_id: Uuid,
        stats: Value,
        error: String,
    ) -> Result<(), PersistenceError> {
        self.finish(run_id, Err(format!("{error}; stats={stats}")))
            .await
    }

    async fn fail_interrupted_before(
        &self,
        job: &str,
        started_before: DateTime<FixedOffset>,
    ) -> Result<u64, PersistenceError>;
}

pub type SharedIngestRunLog = std::sync::Arc<dyn IngestRunLog + Send + Sync>;
