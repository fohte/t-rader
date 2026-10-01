use std::collections::HashSet;

use async_trait::async_trait;
use core_application::strategy_scope::{StrategyScopeSource, StrategyScopeSourceError};
use gateway_postgres::entities::strategy;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use uuid::Uuid;

use crate::DatabaseHandle;

pub struct PostgresStrategyScopeSource {
    connection: DatabaseHandle,
}

impl PostgresStrategyScopeSource {
    pub fn new(connection: DatabaseHandle) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl StrategyScopeSource for PostgresStrategyScopeSource {
    async fn existing_ids(&self, ids: &[Uuid]) -> Result<HashSet<Uuid>, StrategyScopeSourceError> {
        if ids.is_empty() {
            return Ok(HashSet::new());
        }

        strategy::Entity::find()
            .select_only()
            .column(strategy::Column::Id)
            .filter(strategy::Column::Id.is_in(ids.iter().copied()))
            .into_tuple::<Uuid>()
            .all(&self.connection)
            .await
            .map(|ids| ids.into_iter().collect())
            .map_err(|error| StrategyScopeSourceError::QueryFailed(error.to_string()))
    }
}
