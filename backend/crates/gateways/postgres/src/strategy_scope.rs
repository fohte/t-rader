use std::collections::HashSet;

use async_trait::async_trait;
use core_application::strategy_scope::{StrategyScopeSource, StrategyScopeSourceError};
use gateway_postgres::entities::strategy;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect};
use uuid::Uuid;

pub struct PostgresStrategyScopeSource<'a, C> {
    connection: &'a C,
}

impl<'a, C> PostgresStrategyScopeSource<'a, C> {
    pub fn new(connection: &'a C) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl<C> StrategyScopeSource for PostgresStrategyScopeSource<'_, C>
where
    C: ConnectionTrait + Sync,
{
    async fn existing_ids(&self, ids: &[Uuid]) -> Result<HashSet<Uuid>, StrategyScopeSourceError> {
        if ids.is_empty() {
            return Ok(HashSet::new());
        }

        strategy::Entity::find()
            .select_only()
            .column(strategy::Column::Id)
            .filter(strategy::Column::Id.is_in(ids.iter().copied()))
            .into_tuple::<Uuid>()
            .all(self.connection)
            .await
            .map(|ids| ids.into_iter().collect())
            .map_err(|error| StrategyScopeSourceError::QueryFailed(error.to_string()))
    }
}
