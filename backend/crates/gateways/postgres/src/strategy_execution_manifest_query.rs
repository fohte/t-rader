use async_trait::async_trait;
use core_application::strategy_execution::{
    PayloadManifest, PayloadManifestCursor, StrategyExecutionManifestQueryRepository,
    StrategyExecutionRepositoryError,
};
use sea_orm::{
    ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::Condition,
};
use uuid::Uuid;

use crate::{DatabaseHandle, entities::task_execution_payload, persistence::persistence_error};

#[derive(Clone)]
pub struct PostgresStrategyExecutionManifestQueryRepository {
    db: DatabaseHandle,
}

impl PostgresStrategyExecutionManifestQueryRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl StrategyExecutionManifestQueryRepository for PostgresStrategyExecutionManifestQueryRepository {
    async fn count_payload_manifests(
        &self,
        task_id: Uuid,
    ) -> Result<u64, StrategyExecutionRepositoryError> {
        task_execution_payload::Entity::find()
            .filter(task_execution_payload::Column::TaskId.eq(task_id))
            .count(&self.db)
            .await
            .map_err(|error| StrategyExecutionRepositoryError::Database(persistence_error(error)))
    }

    async fn list_payload_manifests(
        &self,
        task_id: Uuid,
        cursor: Option<PayloadManifestCursor>,
        limit: usize,
    ) -> Result<Vec<PayloadManifest>, StrategyExecutionRepositoryError> {
        let mut query = task_execution_payload::Entity::find()
            .filter(task_execution_payload::Column::TaskId.eq(task_id));
        if let Some(cursor) = cursor {
            let created_at = cursor.created_at;
            query = query.filter(
                Condition::any()
                    .add(task_execution_payload::Column::CreatedAt.gt(created_at))
                    .add(
                        Condition::all()
                            .add(task_execution_payload::Column::CreatedAt.eq(created_at))
                            .add(task_execution_payload::Column::Id.gt(cursor.id)),
                    ),
            );
        }
        let limit =
            u64::try_from(limit).map_err(|_| StrategyExecutionRepositoryError::PayloadTooLarge)?;
        let rows = query
            .order_by_asc(task_execution_payload::Column::CreatedAt)
            .order_by_asc(task_execution_payload::Column::Id)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(|error| {
                StrategyExecutionRepositoryError::Database(persistence_error(error))
            })?;
        rows.into_iter().map(TryInto::try_into).collect()
    }
}
