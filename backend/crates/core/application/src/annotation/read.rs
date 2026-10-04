use thiserror::Error;
use uuid::Uuid;

use super::ports::Annotation;
use super::query::{
    AnnotationListQuery, AnnotationReadQueryError, RecentAnnotation, SharedAnnotationReadQuery,
};

#[derive(Debug, Error)]
pub enum AnnotationReadUseCaseError {
    #[error("annotation {0} not found")]
    NotFound(Uuid),
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

    pub async fn get_annotation(&self, id: Uuid) -> Result<Annotation, AnnotationReadUseCaseError> {
        let annotation = self
            .query
            .find_by_id(id)
            .await?
            .ok_or(AnnotationReadUseCaseError::NotFound(id))?;
        Ok(annotation)
    }

    pub async fn list_annotations(
        &self,
        query: AnnotationListQuery,
    ) -> Result<Vec<Annotation>, AnnotationReadUseCaseError> {
        Ok(self.query.list(query).await?)
    }

    pub async fn list_recent_annotations(
        &self,
        limit: u64,
    ) -> Result<Vec<RecentAnnotation>, AnnotationReadUseCaseError> {
        Ok(self.query.list_recent(limit).await?)
    }
}
