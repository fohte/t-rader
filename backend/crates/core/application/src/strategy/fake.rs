use std::collections::HashMap;

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::query::{StrategySummaryQuery, StrategySummaryQueryError};
use super::repository::{StrategyRepository, StrategyRepositoryError};
use super::types::{InvestableAmount, NewInvestableAmount, NewStrategy, Strategy, StrategySummary};

#[derive(Default)]
pub struct FakeStrategyRepository {
    pub strategies: Mutex<HashMap<Uuid, Strategy>>,
    pub amounts: Mutex<Vec<InvestableAmount>>,
    pub transaction_ids: Mutex<Vec<Uuid>>,
}

impl FakeStrategyRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert_strategy(&self, strategy: Strategy) {
        self.strategies.lock().await.insert(strategy.id, strategy);
    }

    async fn record_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<(), StrategyRepositoryError> {
        let transaction_id = transaction
            .downcast_ref::<FakeTransaction>()
            .map(|transaction| transaction.id)
            .ok_or(StrategyRepositoryError::InvalidTransaction)?;
        self.transaction_ids.lock().await.push(transaction_id);
        Ok(())
    }
}

#[async_trait]
impl StrategyRepository for FakeStrategyRepository {
    async fn list(&self) -> Result<Vec<Strategy>, StrategyRepositoryError> {
        let mut rows: Vec<Strategy> = self.strategies.lock().await.values().cloned().collect();
        rows.sort_by(|left, right| {
            left.sort_order
                .cmp(&right.sort_order)
                .then_with(|| left.created_at.cmp(&right.created_at))
        });
        Ok(rows)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Strategy>, StrategyRepositoryError> {
        Ok(self.strategies.lock().await.get(&id).cloned())
    }

    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<Strategy>, StrategyRepositoryError> {
        self.record_transaction(transaction).await?;
        self.find_by_id(id).await
    }

    async fn create(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy: NewStrategy,
    ) -> Result<Strategy, StrategyRepositoryError> {
        self.record_transaction(transaction).await?;
        let now = Utc::now().fixed_offset();
        let strategy = Strategy {
            id: strategy.id,
            name: strategy.name,
            description: strategy.description,
            sort_order: strategy.sort_order,
            created_at: now,
            updated_at: now,
        };
        self.strategies
            .lock()
            .await
            .insert(strategy.id, strategy.clone());
        Ok(strategy)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy: Strategy,
    ) -> Result<Strategy, StrategyRepositoryError> {
        self.record_transaction(transaction).await?;
        self.strategies
            .lock()
            .await
            .insert(strategy.id, strategy.clone());
        Ok(strategy)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, StrategyRepositoryError> {
        self.record_transaction(transaction).await?;
        Ok(self.strategies.lock().await.remove(&id).is_some())
    }

    async fn find_current_investable_amount(
        &self,
        strategy_id: Uuid,
        as_of: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<Option<InvestableAmount>, StrategyRepositoryError> {
        Ok(self
            .amounts
            .lock()
            .await
            .iter()
            .filter(|amount| amount.strategy_id == strategy_id && amount.effective_at <= as_of)
            .max_by(|left, right| {
                left.effective_at
                    .cmp(&right.effective_at)
                    .then_with(|| left.created_at.cmp(&right.created_at))
            })
            .cloned())
    }

    async fn record_investable_amount(
        &self,
        transaction: &UnitOfWorkTransaction,
        amount: NewInvestableAmount,
    ) -> Result<InvestableAmount, StrategyRepositoryError> {
        self.record_transaction(transaction).await?;
        let amount = InvestableAmount {
            id: amount.id,
            strategy_id: amount.strategy_id,
            amount_jpy: amount.amount_jpy,
            effective_at: amount.effective_at,
            created_at: Utc::now().fixed_offset(),
        };
        self.amounts.lock().await.push(amount.clone());
        Ok(amount)
    }
}

#[derive(Default)]
pub struct FakeStrategySummaryQuery {
    pub summaries: Mutex<Vec<StrategySummary>>,
}

impl FakeStrategySummaryQuery {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl StrategySummaryQuery for FakeStrategySummaryQuery {
    async fn list(&self) -> Result<Vec<StrategySummary>, StrategySummaryQueryError> {
        Ok(self.summaries.lock().await.clone())
    }
}
