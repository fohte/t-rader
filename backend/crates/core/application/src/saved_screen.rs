use chrono::{DateTime, FixedOffset};
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::screener::{ScreenerFilterValidationError, ScreenerFilters, ScreenerSort};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedScreen {
    pub id: Uuid,
    pub name: String,
    pub filters: ScreenerFilters,
    pub sort: ScreenerSort,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedScreenData {
    pub name: String,
    pub filters: ScreenerFilters,
    pub sort: ScreenerSort,
}

#[derive(Debug, Error)]
pub enum SavedScreenRepositoryError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error("saved screen with name '{0}' already exists")]
    DuplicateName(String),
    #[error("invalid saved screen data: {0}")]
    InvalidData(String),
}

#[async_trait::async_trait]
pub trait SavedScreenRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<SavedScreen>, SavedScreenRepositoryError>;
    async fn create(
        &self,
        data: SavedScreenData,
    ) -> Result<SavedScreen, SavedScreenRepositoryError>;
    async fn update(
        &self,
        id: Uuid,
        data: SavedScreenData,
    ) -> Result<Option<SavedScreen>, SavedScreenRepositoryError>;
    async fn delete(&self, id: Uuid) -> Result<bool, SavedScreenRepositoryError>;
}

pub type SharedSavedScreenRepository = std::sync::Arc<dyn SavedScreenRepository + Send + Sync>;

#[derive(Debug, Error)]
pub enum SavedScreenUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("saved screen {0} not found")]
    NotFound(Uuid),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Repository(#[from] SavedScreenRepositoryError),
}

#[derive(Clone)]
pub struct SavedScreenUseCases {
    repository: SharedSavedScreenRepository,
}

impl SavedScreenUseCases {
    pub fn new(repository: SharedSavedScreenRepository) -> Self {
        Self { repository }
    }

    pub async fn list(&self) -> Result<Vec<SavedScreen>, SavedScreenUseCaseError> {
        self.repository.list().await.map_err(Into::into)
    }

    pub async fn create(
        &self,
        data: SavedScreenData,
    ) -> Result<SavedScreen, SavedScreenUseCaseError> {
        let data = validate(data)?;
        self.repository
            .create(data)
            .await
            .map_err(map_repository_error)
    }

    pub async fn update(
        &self,
        id: Uuid,
        data: SavedScreenData,
    ) -> Result<SavedScreen, SavedScreenUseCaseError> {
        let data = validate(data)?;
        self.repository
            .update(id, data)
            .await
            .map_err(map_repository_error)?
            .ok_or(SavedScreenUseCaseError::NotFound(id))
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), SavedScreenUseCaseError> {
        if self.repository.delete(id).await? {
            Ok(())
        } else {
            Err(SavedScreenUseCaseError::NotFound(id))
        }
    }
}

fn validate(mut data: SavedScreenData) -> Result<SavedScreenData, SavedScreenUseCaseError> {
    data.name = data.name.trim().to_owned();
    if data.name.is_empty() {
        return Err(SavedScreenUseCaseError::Validation(
            "name must not be empty".to_owned(),
        ));
    }
    data.filters
        .validate()
        .map_err(|error: ScreenerFilterValidationError| {
            SavedScreenUseCaseError::Validation(error.to_string())
        })?;
    Ok(data)
}

fn map_repository_error(error: SavedScreenRepositoryError) -> SavedScreenUseCaseError {
    match error {
        SavedScreenRepositoryError::DuplicateName(name) => {
            SavedScreenUseCaseError::Conflict(format!("saved screen '{name}' already exists"))
        }
        error => SavedScreenUseCaseError::Repository(error),
    }
}
