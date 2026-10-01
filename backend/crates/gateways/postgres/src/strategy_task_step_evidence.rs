use async_trait::async_trait;
use core_application::strategy_task_step_evidence::{
    StrategyTaskStepEvidence, StrategyTaskStepEvidenceRepository,
    StrategyTaskStepEvidenceRepositoryError,
};
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;

use crate::DatabaseHandle;
use crate::entities::strategy_task_step_evidence;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresStrategyTaskStepEvidenceRepository {
    db: DatabaseHandle,
}

impl PostgresStrategyTaskStepEvidenceRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl StrategyTaskStepEvidenceRepository for PostgresStrategyTaskStepEvidenceRepository {
    async fn insert(
        &self,
        evidence: StrategyTaskStepEvidence,
    ) -> Result<(), StrategyTaskStepEvidenceRepositoryError> {
        strategy_task_step_evidence::ActiveModel {
            id: Set(evidence.id),
            execution_step_id: Set(evidence.execution_step_id),
            source: Set(evidence.source),
            source_ref: Set(evidence.source_ref),
            observed_at: Set(evidence.observed_at),
            published_at: Set(evidence.published_at),
            effective_at: Set(evidence.effective_at),
            snapshot: Set(evidence.snapshot),
        }
        .insert(&self.db)
        .await
        .map(|_| ())
        .map_err(|error| {
            StrategyTaskStepEvidenceRepositoryError::Database(persistence_error(error))
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use core_application::strategy_task_step_evidence::StrategyTaskStepEvidence;
    use sea_orm::EntityTrait;
    use serde_json::json;
    use uuid::Uuid;

    use super::PostgresStrategyTaskStepEvidenceRepository;
    use crate::entities::strategy_task_step_evidence;
    use crate::entities::strategy_task_step_evidence::Entity as EvidenceEntity;
    use core_application::strategy_task_step_evidence::StrategyTaskStepEvidenceRepository;

    #[backend_test_macros::database_test]
    async fn repository_inserts_evidence(db: crate::DatabaseHandle) {
        let evidence = StrategyTaskStepEvidence {
            id: Uuid::new_v4(),
            execution_step_id: Uuid::new_v4(),
            source: "query_data".to_string(),
            source_ref: "fictional-instrument".to_string(),
            observed_at: DateTime::parse_from_rfc3339("2030-01-02T00:00:00Z").expect("observed at"),
            published_at: Some(
                DateTime::parse_from_rfc3339("2030-01-01T00:00:00Z").expect("published at"),
            ),
            effective_at: Some(
                DateTime::parse_from_rfc3339("2030-01-01T00:00:00Z").expect("effective at"),
            ),
            snapshot: json!({"sample": "fictional"}),
        };
        let expected = strategy_task_step_evidence::Model {
            id: evidence.id,
            execution_step_id: evidence.execution_step_id,
            source: evidence.source.clone(),
            source_ref: evidence.source_ref.clone(),
            observed_at: evidence.observed_at,
            published_at: evidence.published_at,
            effective_at: evidence.effective_at,
            snapshot: evidence.snapshot.clone(),
        };

        PostgresStrategyTaskStepEvidenceRepository::new(db.clone())
            .insert(evidence)
            .await
            .expect("insert evidence");

        let actual = EvidenceEntity::find_by_id(expected.id)
            .one(&db)
            .await
            .expect("fetch evidence");
        assert_eq!(actual, Some(expected));
    }
}
