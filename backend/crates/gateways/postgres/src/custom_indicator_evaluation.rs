use async_trait::async_trait;
use core_application::custom_indicator_evaluation::{
    CustomIndicatorEvaluation, CustomIndicatorEvaluationRepository,
    CustomIndicatorEvaluationRepositoryError,
};
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;

use crate::DatabaseHandle;
use crate::entities::custom_indicator_evaluation;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresCustomIndicatorEvaluationRepository {
    db: DatabaseHandle,
}

impl PostgresCustomIndicatorEvaluationRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CustomIndicatorEvaluationRepository for PostgresCustomIndicatorEvaluationRepository {
    async fn insert(
        &self,
        evaluation: CustomIndicatorEvaluation,
    ) -> Result<(), CustomIndicatorEvaluationRepositoryError> {
        custom_indicator_evaluation::ActiveModel {
            id: Set(evaluation.id),
            indicator_id: Set(evaluation.indicator_id),
            indicator_name: Set(evaluation.indicator_name),
            code: Set(evaluation.code),
            args: Set(evaluation.args),
            output: Set(evaluation.output),
            evaluated_at: Set(evaluation.evaluated_at),
        }
        .insert(&self.db)
        .await
        .map(|_| ())
        .map_err(|error| {
            CustomIndicatorEvaluationRepositoryError::Database(persistence_error(error))
        })
    }
}
