use thiserror::Error;
use uuid::Uuid;

use crate::strategy_scope::StrategyScope;

use super::ports::Annotation;
use super::query::{
    AnnotationListQuery, AnnotationReadQueryError, RecentAnnotation, SharedAnnotationReadQuery,
};

#[derive(Debug, Error)]
pub enum AnnotationReadUseCaseError {
    #[error("annotation {0} not found")]
    NotFound(Uuid),
    #[error("annotation {0} belongs to another strategy")]
    Forbidden(Uuid),
    #[error(transparent)]
    Query(#[from] AnnotationReadQueryError),
}

#[derive(Clone)]
pub struct AnnotationReadUseCases {
    query: SharedAnnotationReadQuery,
}

impl AnnotationReadUseCases {
    pub fn new(query: SharedAnnotationReadQuery) -> Self {
        Self { query }
    }

    pub async fn get_annotation(
        &self,
        id: Uuid,
        scope: Option<StrategyScope>,
    ) -> Result<Annotation, AnnotationReadUseCaseError> {
        let annotation = self
            .query
            .find_by_id(id)
            .await?
            .ok_or(AnnotationReadUseCaseError::NotFound(id))?;
        if let Some(scope) = scope
            && annotation.strategy_id != Some(scope.id())
        {
            return Err(AnnotationReadUseCaseError::Forbidden(id));
        }
        Ok(annotation)
    }

    pub async fn list_annotations(
        &self,
        mut query: AnnotationListQuery,
        scope: Option<StrategyScope>,
    ) -> Result<Vec<Annotation>, AnnotationReadUseCaseError> {
        if let Some(scope) = scope {
            query.strategy_id = Some(scope.id());
        }
        Ok(self.query.list(query).await?)
    }

    pub async fn list_recent_annotations(
        &self,
        strategy_id: Uuid,
        limit: u64,
    ) -> Result<Vec<RecentAnnotation>, AnnotationReadUseCaseError> {
        Ok(self.query.list_recent(strategy_id, limit).await?)
    }
}
