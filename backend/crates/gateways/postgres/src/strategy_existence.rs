use async_trait::async_trait;
use core_application::strategy_existence::{StrategyExistence, StrategyExistenceError};
use sea_orm::EntityTrait;
use uuid::Uuid;

use crate::entities::strategy;
use crate::transaction::transaction_ref;

#[derive(Clone, Copy, Default)]
pub struct PostgresStrategyExistence;

#[async_trait]
impl StrategyExistence for PostgresStrategyExistence {
    async fn exists(
        &self,
        transaction: &core_application::unit_of_work::UnitOfWorkTransaction,
        strategy_id: Uuid,
    ) -> Result<bool, StrategyExistenceError> {
        let transaction =
            transaction_ref(transaction).ok_or(StrategyExistenceError::InvalidTransaction)?;
        strategy::Entity::find_by_id(strategy_id)
            .one(transaction)
            .await
            .map(|row| row.is_some())
            .map_err(|error| {
                StrategyExistenceError::Database(crate::persistence::persistence_error(error))
            })
    }
}
