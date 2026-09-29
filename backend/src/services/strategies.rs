use core_application::strategy_scope::{
    StrategyScope, StrategyScopeError, StrategyScopeSourceError, verify_strategy_ids,
};
use gateway_postgres::PostgresStrategyScopeSource;
use sea_orm::ConnectionTrait;
use uuid::Uuid;

use crate::error::AppError;

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
    let source = PostgresStrategyScopeSource::new(conn);
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
    let source = PostgresStrategyScopeSource::new(conn);
    verify_strategy_ids(ids, &source)
        .await
        .map_err(map_scope_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::insert_test_strategy;
    use core_application::strategy_scope::StrategyScopeSource;
    use std::collections::HashSet;

    #[backend_test_macros::database_test]
    async fn database_source_returns_only_existing_ids(db: gateway_postgres::DatabaseHandle) {
        let existing_id = insert_test_strategy(&db, "scope test").await;
        let missing_id = Uuid::new_v4();
        let source = PostgresStrategyScopeSource::new(&db);

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
