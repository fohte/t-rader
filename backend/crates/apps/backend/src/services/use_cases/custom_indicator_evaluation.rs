use std::sync::Arc;

use core_application::custom_indicator_evaluation::CustomIndicatorEvaluationUseCases;
use gateway_postgres::PostgresCustomIndicatorEvaluationRepository;

use super::UseCases;

impl UseCases {
    pub fn custom_indicator_evaluations(&self) -> CustomIndicatorEvaluationUseCases {
        CustomIndicatorEvaluationUseCases::new(Arc::new(
            PostgresCustomIndicatorEvaluationRepository::new(self.db.clone()),
        ))
    }
}
