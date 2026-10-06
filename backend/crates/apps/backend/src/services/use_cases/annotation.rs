use std::sync::Arc;

use core_application::annotation::{AnnotationReadUseCases, AnnotationUseCases};
use gateway_postgres::{
    PostgresAnnotationReadQuery, PostgresAnnotationRepository,
    PostgresStrategyTaskStepEvidenceRepository,
};

use super::UseCases;

impl UseCases {
    pub fn annotation_reads(&self) -> AnnotationReadUseCases {
        AnnotationReadUseCases::new(Arc::new(PostgresAnnotationReadQuery::new(
            self.db.clone(),
        )))
    }

    pub fn annotations(&self) -> AnnotationUseCases {
        AnnotationUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresAnnotationRepository),
            self.change_history.clone(),
            Arc::new(PostgresStrategyTaskStepEvidenceRepository::new(
                self.db.clone(),
            )),
        )
    }
}
