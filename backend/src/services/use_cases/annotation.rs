use std::sync::Arc;

use core_application::annotation::AnnotationUseCases;
use gateway_postgres::PostgresAnnotationRepository;

use super::UseCases;

impl UseCases {
    pub fn annotations(&self) -> AnnotationUseCases {
        AnnotationUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresAnnotationRepository),
            self.strategy_existence.clone(),
            self.change_history.clone(),
        )
    }
}
