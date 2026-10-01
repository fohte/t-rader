use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;

use super::ports::Annotation;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnnotationListQuery {
    pub strategy_id: Option<Uuid>,
    pub target_symbol: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentAnnotation {
    pub id: Uuid,
    pub target_symbol: String,
    pub target_kind: String,
    pub status: String,
    pub created_by_kind: String,
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
}

#[derive(Debug, Error)]
pub enum AnnotationReadQueryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait AnnotationReadQuery: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Annotation>, AnnotationReadQueryError>;

    async fn list(
        &self,
        query: AnnotationListQuery,
    ) -> Result<Vec<Annotation>, AnnotationReadQueryError>;

    async fn list_recent(
        &self,
        strategy_id: Uuid,
        limit: u64,
    ) -> Result<Vec<RecentAnnotation>, AnnotationReadQueryError>;
}

pub type SharedAnnotationReadQuery = Arc<dyn AnnotationReadQuery + Send + Sync>;
