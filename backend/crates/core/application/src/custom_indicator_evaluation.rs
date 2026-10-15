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

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<CustomIndicatorEvaluation>, CustomIndicatorEvaluationRepositoryError>;
}

pub type SharedCustomIndicatorEvaluationRepository =
    Arc<dyn CustomIndicatorEvaluationRepository + Send + Sync>;

#[derive(Debug, Error)]
pub enum CustomIndicatorEvaluationUseCaseError {
    #[error(transparent)]
    Repository(#[from] CustomIndicatorEvaluationRepositoryError),
}

pub struct RecordCustomIndicatorEvaluationCommand {
    pub indicator_id: Uuid,
    pub indicator_name: String,
    pub code: String,
    pub args: JsonValue,
    pub output: JsonValue,
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
        command: RecordCustomIndicatorEvaluationCommand,
    ) -> Result<Uuid, CustomIndicatorEvaluationUseCaseError> {
        let evaluation = CustomIndicatorEvaluation {
            id: Uuid::new_v4(),
            indicator_id: command.indicator_id,
            indicator_name: command.indicator_name,
            code: command.code,
            args: command.args,
            output: command.output,
            evaluated_at: Utc::now().fixed_offset(),
        };
        let id = evaluation.id;
        self.repository.insert(evaluation).await?;
        Ok(id)
    }

    pub async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<CustomIndicatorEvaluation>, CustomIndicatorEvaluationUseCaseError> {
        Ok(self.repository.find_by_id(id).await?)
    }
}
