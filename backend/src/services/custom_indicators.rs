use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entities::custom_indicator;
use crate::error::AppError;

pub const SCOPE_GLOBAL: &str = "global";
pub const SCOPE_STRATEGY: &str = "strategy";

/// 戦略 scope の同名 indicator があれば優先し、無ければ global を返す
pub async fn resolve_indicator<C: ConnectionTrait>(
    conn: &C,
    strategy_id: Uuid,
    name: &str,
) -> Result<Option<custom_indicator::Model>, AppError> {
    let strategy_scoped = custom_indicator::Entity::find()
        .filter(custom_indicator::Column::Scope.eq(SCOPE_STRATEGY))
        .filter(custom_indicator::Column::StrategyId.eq(strategy_id))
        .filter(custom_indicator::Column::Name.eq(name))
        .one(conn)
        .await?;
    if strategy_scoped.is_some() {
        return Ok(strategy_scoped);
    }

    let global = custom_indicator::Entity::find()
        .filter(custom_indicator::Column::Scope.eq(SCOPE_GLOBAL))
        .filter(custom_indicator::Column::Name.eq(name))
        .one(conn)
        .await?;
    Ok(global)
}
