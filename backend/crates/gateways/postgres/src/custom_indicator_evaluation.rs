use async_trait::async_trait;
use core_application::custom_indicator_evaluation::{
    CustomIndicatorEvaluation, CustomIndicatorEvaluationRepository,
    CustomIndicatorEvaluationRepositoryError,
};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, EntityTrait};

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

    async fn find_by_id(
        &self,
        id: uuid::Uuid,
    ) -> Result<Option<CustomIndicatorEvaluation>, CustomIndicatorEvaluationRepositoryError> {
        custom_indicator_evaluation::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map(|evaluation| evaluation.map(Into::into))
            .map_err(|error| {
                CustomIndicatorEvaluationRepositoryError::Database(persistence_error(error))
            })
    }
}

impl From<custom_indicator_evaluation::Model> for CustomIndicatorEvaluation {
    fn from(model: custom_indicator_evaluation::Model) -> Self {
        Self {
            id: model.id,
            indicator_id: model.indicator_id,
            indicator_name: model.indicator_name,
            code: model.code,
            args: model.args,
            output: model.output,
            evaluated_at: model.evaluated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, TimeZone, Utc};
    use core_application::custom_indicator_evaluation::CustomIndicatorEvaluationRepository;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use serde_json::json;
    use uuid::Uuid;

    use crate::DatabaseHandle;
    use crate::entities::custom_indicator_evaluation;

    use super::PostgresCustomIndicatorEvaluationRepository;

    fn evaluated_at() -> DateTime<FixedOffset> {
        Utc.timestamp_opt(1_900_000_000, 0)
            .single()
            .expect("valid timestamp")
            .fixed_offset()
    }

    async fn insert_evaluation(db: &DatabaseHandle, id: Uuid) {
        custom_indicator_evaluation::ActiveModel {
            id: Set(id),
            indicator_id: Set(Uuid::from_u128(0x202)),
            indicator_name: Set("demo-indicator".into()),
            code: Set("return 1".into()),
            args: Set(json!({})),
            output: Set(json!({"result": 18.5})),
            evaluated_at: Set(evaluated_at()),
        }
        .insert(db)
        .await
        .expect("insert evaluation");
    }

    #[backend_test_macros::database_test]
    async fn find_by_id_returns_the_evaluation(db: DatabaseHandle) {
        let id = Uuid::from_u128(0x201);
        insert_evaluation(&db, id).await;
        let repository = PostgresCustomIndicatorEvaluationRepository::new(db);

        let result = repository
            .find_by_id(id)
            .await
            .map_err(|error| error.to_string());

        assert_eq!(
            result,
            Ok(Some(
                core_application::custom_indicator_evaluation::CustomIndicatorEvaluation {
                    id,
                    indicator_id: Uuid::from_u128(0x202),
                    indicator_name: "demo-indicator".into(),
                    code: "return 1".into(),
                    args: json!({}),
                    output: json!({"result": 18.5}),
                    evaluated_at: evaluated_at(),
                }
            )),
        );
    }

    #[backend_test_macros::database_test]
    async fn find_by_id_returns_none_when_missing(db: DatabaseHandle) {
        let repository = PostgresCustomIndicatorEvaluationRepository::new(db);

        let result = repository
            .find_by_id(Uuid::from_u128(0x203))
            .await
            .map_err(|error| error.to_string());

        assert_eq!(result, Ok(None));
    }
}
