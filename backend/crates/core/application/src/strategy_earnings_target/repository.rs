use async_trait::async_trait;
use uuid::Uuid;

use crate::unit_of_work::UnitOfWorkTransaction;

use super::error::StrategyEarningsTargetRepositoryError;
use super::types::StrategyEarningsTarget;

#[async_trait]
pub trait StrategyEarningsTargetRepository: Send + Sync {
    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
        ref_kind: &str,
        ref_id: &str,
    ) -> Result<bool, StrategyEarningsTargetRepositoryError>;

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
        ref_kind: &str,
        ref_id: &str,
    ) -> Result<bool, StrategyEarningsTargetRepositoryError>;

    async fn list(
        &self,
        strategy_id: Uuid,
    ) -> Result<Vec<StrategyEarningsTarget>, StrategyEarningsTargetRepositoryError>;
}

pub type SharedStrategyEarningsTargetRepository =
    std::sync::Arc<dyn StrategyEarningsTargetRepository + Send + Sync>;
