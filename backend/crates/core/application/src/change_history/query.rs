use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;

#[derive(Debug, Clone, PartialEq)]
pub struct ChangeHistoryEntry {
    pub id: Uuid,
    pub target_kind: String,
    pub target_id: Uuid,
    pub actor_kind: String,
    pub actor_label: String,
    pub op: String,
    pub diff_json: Value,
    pub summary: Option<String>,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeHistoryListQuery {
    pub target_kind: Option<String>,
    pub target_id: Option<Uuid>,
    pub limit: u64,
}

#[derive(Debug, Error)]
pub enum ChangeHistoryQueryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait ChangeHistoryQuery: Send + Sync {
    async fn list(
        &self,
        query: ChangeHistoryListQuery,
    ) -> Result<Vec<ChangeHistoryEntry>, ChangeHistoryQueryError>;

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<ChangeHistoryEntry>, ChangeHistoryQueryError>;
}

pub type SharedChangeHistoryQuery = Arc<dyn ChangeHistoryQuery + Send + Sync>;
