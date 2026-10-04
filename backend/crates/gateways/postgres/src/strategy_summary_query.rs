use async_trait::async_trait;
use core_application::strategy::{
    StrategySummary, StrategySummaryQuery, StrategySummaryQueryError,
};
use sea_orm::{EntityTrait, QueryOrder};

use crate::DatabaseHandle;
use crate::entities::strategy;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresStrategySummaryQuery {
    db: DatabaseHandle,
}

impl PostgresStrategySummaryQuery {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl StrategySummaryQuery for PostgresStrategySummaryQuery {
    async fn list(&self) -> Result<Vec<StrategySummary>, StrategySummaryQueryError> {
        let strategies = strategy::Entity::find()
            .order_by_asc(strategy::Column::SortOrder)
            .order_by_asc(strategy::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(query_error)?;
        Ok(strategies
            .into_iter()
            .map(|row| StrategySummary {
                id: row.id,
                name: row.name,
                updated_at: row.updated_at,
            })
            .collect())
    }
}

fn query_error(error: sea_orm::DbErr) -> StrategySummaryQueryError {
    StrategySummaryQueryError::Database(persistence_error(error))
}
