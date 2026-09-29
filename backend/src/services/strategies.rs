use core_application::{StrategyScope, StrategyScopeError, verify_strategy_ids};
use gateway_postgres::PostgresStrategyScopeSource;
use sea_orm::ConnectionTrait;
use uuid::Uuid;

use crate::error::AppError;

fn map_scope_error(error: StrategyScopeError) -> AppError {
    match error {
        StrategyScopeError::NotFound(id) => {
            AppError::Validation(format!("strategy {id} does not exist"))
        }
        StrategyScopeError::Source(error) => {
            tracing::error!(error = %error, "strategy scope validation failed");
            AppError::Config(error.to_string())
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
