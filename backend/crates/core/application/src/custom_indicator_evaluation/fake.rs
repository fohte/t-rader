use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use uuid::Uuid;

use super::{
    CustomIndicatorEvaluation, CustomIndicatorEvaluationRepository,
    CustomIndicatorEvaluationRepositoryError,
};

#[derive(Default)]
pub struct FakeCustomIndicatorEvaluationRepository {
    evaluations: Mutex<HashMap<Uuid, CustomIndicatorEvaluation>>,
}

impl FakeCustomIndicatorEvaluationRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seed(&self, evaluation: CustomIndicatorEvaluation) {
        self.evaluations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(evaluation.id, evaluation);
    }
}

#[async_trait]
impl CustomIndicatorEvaluationRepository for FakeCustomIndicatorEvaluationRepository {
    async fn insert(
        &self,
        evaluation: CustomIndicatorEvaluation,
    ) -> Result<(), CustomIndicatorEvaluationRepositoryError> {
        self.seed(evaluation);
        Ok(())
    }

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<CustomIndicatorEvaluation>, CustomIndicatorEvaluationRepositoryError> {
        Ok(self
            .evaluations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&id)
            .cloned())
    }
}
