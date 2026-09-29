use std::collections::HashSet;

use async_trait::async_trait;
use core_application::{
    StrategyScope, StrategyScopeError, StrategyScopeSource, StrategyScopeSourceError,
    verify_strategy_ids,
};
use gateway_postgres::entities::strategy;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect};
use uuid::Uuid;

use crate::error::AppError;

pub(crate) struct SeaOrmStrategyScopeSource<'a, C> {
    connection: &'a C,
}

impl<'a, C> SeaOrmStrategyScopeSource<'a, C> {
    pub(crate) fn new(connection: &'a C) -> Self {
        Self { connection }
    }
}

#[async_trait]
impl<C> StrategyScopeSource for SeaOrmStrategyScopeSource<'_, C>
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

fn map_scope_error(error: StrategyScopeError) -> AppError {
    match error {
        StrategyScopeError::NotFound(id) => {
            AppError::Validation(format!("strategy {id} does not exist"))
        }
        StrategyScopeError::Source(StrategyScopeSourceError::QueryFailed(message)) => {
            AppError::Database(sea_orm::DbErr::Custom(message))
        }
    }
}

pub async fn ensure_strategy_exists<C: ConnectionTrait + Sync>(
    conn: &C,
    strategy_id: Uuid,
) -> Result<(), AppError> {
    let source = SeaOrmStrategyScopeSource::new(conn);
    StrategyScope::verify(strategy_id, &source)
        .await
        .map(|_| ())
        .map_err(map_scope_error)
}

pub async fn ensure_strategies_exist<C, I>(conn: &C, ids: I) -> Result<(), AppError>
where
    C: ConnectionTrait + Sync,
    I: IntoIterator<Item = Uuid>,
{
    let source = SeaOrmStrategyScopeSource::new(conn);
    verify_strategy_ids(ids, &source)
        .await
        .map_err(map_scope_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::insert_test_strategy;

    #[backend_test_macros::database_test]
    async fn database_source_returns_only_existing_ids(db: gateway_postgres::DatabaseHandle) {
        let existing_id = insert_test_strategy(&db, "scope test").await;
        let missing_id = Uuid::new_v4();
        let source = SeaOrmStrategyScopeSource::new(&db);

        let result = (
            source.existing_ids(&[existing_id, missing_id]).await,
            source.existing_ids(&[]).await,
        );

        assert_eq!(
            result,
            (Ok(HashSet::from([existing_id])), Ok(HashSet::new()),),
        );
    }
}
