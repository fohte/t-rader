use std::collections::HashSet;

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entities::strategy;
use crate::error::AppError;

pub async fn ensure_strategy_exists<C: ConnectionTrait>(
    conn: &C,
    strategy_id: Uuid,
) -> Result<(), AppError> {
    ensure_strategies_exist(conn, [strategy_id]).await
}

pub async fn ensure_strategies_exist<C: ConnectionTrait, I: IntoIterator<Item = Uuid>>(
    conn: &C,
    ids: I,
) -> Result<(), AppError> {
    let unique: HashSet<Uuid> = ids.into_iter().collect();
    if unique.is_empty() {
        return Ok(());
    }
    let ids: Vec<Uuid> = unique.iter().copied().collect();
    let found: HashSet<Uuid> = strategy::Entity::find()
        .filter(strategy::Column::Id.is_in(ids))
        .all(conn)
        .await?
        .into_iter()
        .map(|s| s.id)
        .collect();
    if let Some(missing) = unique.difference(&found).next() {
        return Err(AppError::Validation(format!(
            "strategy {missing} does not exist"
        )));
    }
    Ok(())
}
