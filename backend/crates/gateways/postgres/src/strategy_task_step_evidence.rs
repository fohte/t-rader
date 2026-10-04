use async_trait::async_trait;
use core_application::strategy_task_step_evidence::{
    StrategyTaskStepEvidence, StrategyTaskStepEvidenceRepository,
    StrategyTaskStepEvidenceRepositoryError,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::strategy_task_step_evidence;
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref;

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

    async fn find_query_data(
        &self,
        transaction: &UnitOfWorkTransaction,
        execution_step_id: Uuid,
        instrument_id: &str,
    ) -> Result<Vec<StrategyTaskStepEvidence>, StrategyTaskStepEvidenceRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(StrategyTaskStepEvidenceRepositoryError::InvalidTransaction)?;
        strategy_task_step_evidence::Entity::find()
            .filter(strategy_task_step_evidence::Column::ExecutionStepId.eq(execution_step_id))
            .filter(strategy_task_step_evidence::Column::Source.eq("query_data"))
            .filter(strategy_task_step_evidence::Column::SourceRef.eq(instrument_id))
            .order_by_asc(strategy_task_step_evidence::Column::ObservedAt)
            .all(transaction)
            .await
            .map(|rows| rows.into_iter().map(to_evidence).collect())
            .map_err(|error| {
                StrategyTaskStepEvidenceRepositoryError::Database(persistence_error(error))
            })
    }

    async fn find_query_data_by_evidence_id(
        &self,
        transaction: &UnitOfWorkTransaction,
        evidence_id: Uuid,
    ) -> Result<Option<StrategyTaskStepEvidence>, StrategyTaskStepEvidenceRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(StrategyTaskStepEvidenceRepositoryError::InvalidTransaction)?;
        strategy_task_step_evidence::Entity::find_by_id(evidence_id)
            .filter(strategy_task_step_evidence::Column::Source.eq("query_data"))
            .one(transaction)
            .await
            .map(|row| row.map(to_evidence))
            .map_err(|error| {
                StrategyTaskStepEvidenceRepositoryError::Database(persistence_error(error))
            })
    }
}

fn to_evidence(model: strategy_task_step_evidence::Model) -> StrategyTaskStepEvidence {
    StrategyTaskStepEvidence {
        id: model.id,
        execution_step_id: model.execution_step_id,
        source: model.source,
        source_ref: model.source_ref,
        observed_at: model.observed_at,
        published_at: model.published_at,
        effective_at: model.effective_at,
        snapshot: model.snapshot,
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use core_application::strategy_task_step_evidence::StrategyTaskStepEvidence;
    use core_application::unit_of_work::UnitOfWorkTransaction;
    use sea_orm::EntityTrait;
    use sea_orm::TransactionTrait;
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

    #[backend_test_macros::database_test]
    async fn repository_finds_matching_query_data_in_observation_order(db: crate::DatabaseHandle) {
        let execution_step_id = Uuid::from_u128(901);
        let matching_earlier = evidence(
            Uuid::from_u128(902),
            execution_step_id,
            "query_data",
            "fictional-instrument",
            "2030-01-01T00:00:00Z",
        );
        let matching_later = evidence(
            Uuid::from_u128(903),
            execution_step_id,
            "query_data",
            "fictional-instrument",
            "2030-01-02T00:00:00Z",
        );
        let decoys = [
            evidence(
                Uuid::from_u128(904),
                Uuid::from_u128(905),
                "query_data",
                "fictional-instrument",
                "2030-01-03T00:00:00Z",
            ),
            evidence(
                Uuid::from_u128(906),
                execution_step_id,
                "other_source",
                "fictional-instrument",
                "2030-01-04T00:00:00Z",
            ),
            evidence(
                Uuid::from_u128(907),
                execution_step_id,
                "query_data",
                "fictional-other",
                "2030-01-05T00:00:00Z",
            ),
        ];
        let repository = PostgresStrategyTaskStepEvidenceRepository::new(db.clone());
        for row in [
            matching_later.clone(),
            decoys[0].clone(),
            matching_earlier.clone(),
            decoys[1].clone(),
            decoys[2].clone(),
        ] {
            repository.insert(row).await.expect("insert evidence");
        }

        let transaction =
            UnitOfWorkTransaction::new(db.begin().await.expect("begin query transaction"));
        let actual = repository
            .find_query_data(&transaction, execution_step_id, "fictional-instrument")
            .await
            .expect("find query data");
        let actual_by_id = repository
            .find_query_data_by_evidence_id(&transaction, matching_earlier.id)
            .await
            .expect("find evidence by id");

        assert_eq!(
            (actual, actual_by_id),
            (
                vec![matching_earlier.clone(), matching_later],
                Some(matching_earlier),
            ),
        );
    }

    fn evidence(
        id: Uuid,
        execution_step_id: Uuid,
        source: &str,
        source_ref: &str,
        observed_at: &str,
    ) -> StrategyTaskStepEvidence {
        StrategyTaskStepEvidence {
            id,
            execution_step_id,
            source: source.to_string(),
            source_ref: source_ref.to_string(),
            observed_at: DateTime::parse_from_rfc3339(observed_at).expect("observed at"),
            published_at: None,
            effective_at: None,
            snapshot: json!({"sample": "fictional"}),
        }
    }
}
