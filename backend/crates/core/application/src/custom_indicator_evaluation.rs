use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;

#[derive(Debug, Clone, PartialEq)]
pub struct CustomIndicatorEvaluation {
    pub id: Uuid,
    pub indicator_id: Uuid,
    pub indicator_name: String,
    pub code: String,
    pub args: JsonValue,
    pub output: JsonValue,
    pub evaluated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Error)]
pub enum CustomIndicatorEvaluationRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait CustomIndicatorEvaluationRepository: Send + Sync {
    async fn insert(
        &self,
        evaluation: CustomIndicatorEvaluation,
    ) -> Result<(), CustomIndicatorEvaluationRepositoryError>;
}

pub type SharedCustomIndicatorEvaluationRepository =
    Arc<dyn CustomIndicatorEvaluationRepository + Send + Sync>;

#[derive(Debug, Error)]
pub enum CustomIndicatorEvaluationUseCaseError {
    #[error(transparent)]
    Repository(#[from] CustomIndicatorEvaluationRepositoryError),
}

#[derive(Clone)]
pub struct CustomIndicatorEvaluationUseCases {
    repository: SharedCustomIndicatorEvaluationRepository,
}

impl CustomIndicatorEvaluationUseCases {
    pub fn new(repository: SharedCustomIndicatorEvaluationRepository) -> Self {
        Self { repository }
    }

    pub async fn record(
        &self,
        indicator_id: Uuid,
        indicator_name: String,
        code: String,
        args: JsonValue,
        output: JsonValue,
    ) -> Result<Uuid, CustomIndicatorEvaluationUseCaseError> {
        let evaluation = CustomIndicatorEvaluation {
            id: Uuid::new_v4(),
            indicator_id,
            indicator_name,
            code,
            args,
            output,
            evaluated_at: Utc::now().fixed_offset(),
        };
        let id = evaluation.id;
        self.repository.insert(evaluation).await?;
        Ok(id)
    }
}
