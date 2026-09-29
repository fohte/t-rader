use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::repository::{CustomIndicatorRepository, CustomIndicatorRepositoryError};
use super::types::{CustomIndicator, NewCustomIndicator, SCOPE_GLOBAL, SCOPE_STRATEGY};

#[derive(Default)]
pub struct FakeCustomIndicatorRepository {
    pub indicators: Mutex<HashMap<Uuid, CustomIndicator>>,
    pub transaction_ids: Mutex<Vec<Uuid>>,
}

impl FakeCustomIndicatorRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl CustomIndicatorRepository for FakeCustomIndicatorRepository {
    async fn list_global(&self) -> Result<Vec<CustomIndicator>, CustomIndicatorRepositoryError> {
        let mut indicators: Vec<_> = self
            .indicators
            .lock()
            .await
            .values()
            .filter(|indicator| indicator.scope == SCOPE_GLOBAL)
            .cloned()
            .collect();
        indicators.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(indicators)
    }

    async fn list_strategy(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
    ) -> Result<Vec<CustomIndicator>, CustomIndicatorRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut indicators: Vec<_> = self
            .indicators
            .lock()
            .await
            .values()
            .filter(|indicator| {
                indicator.scope == SCOPE_STRATEGY && indicator.strategy_id == Some(strategy_id)
            })
            .cloned()
            .collect();
        indicators.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(indicators)
    }

    async fn find_by_id(
        &self,
        indicator_id: Uuid,
    ) -> Result<Option<CustomIndicator>, CustomIndicatorRepositoryError> {
        Ok(self.indicators.lock().await.get(&indicator_id).cloned())
    }

    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        indicator_id: Uuid,
    ) -> Result<Option<CustomIndicator>, CustomIndicatorRepositoryError> {
        self.record_transaction(transaction).await?;
        self.find_by_id(indicator_id).await
    }

    async fn resolve_for_strategy(
        &self,
        strategy_id: Uuid,
        name: &str,
    ) -> Result<Option<CustomIndicator>, CustomIndicatorRepositoryError> {
        let indicators = self.indicators.lock().await;
        Ok(indicators
            .values()
            .find(|indicator| {
                indicator.scope == SCOPE_STRATEGY
                    && indicator.strategy_id == Some(strategy_id)
                    && indicator.name == name
            })
            .or_else(|| {
                indicators
                    .values()
                    .find(|indicator| indicator.scope == SCOPE_GLOBAL && indicator.name == name)
            })
            .cloned())
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        indicator: NewCustomIndicator,
    ) -> Result<CustomIndicator, CustomIndicatorRepositoryError> {
        self.record_transaction(transaction).await?;
        let now = chrono::Utc::now().fixed_offset();
        let indicator = CustomIndicator {
            indicator_id: indicator.indicator_id,
            name: indicator.name,
            scope: indicator.scope,
            strategy_id: indicator.strategy_id,
            code: indicator.code,
            input_schema: indicator.input_schema,
            output_schema: indicator.output_schema,
            description: indicator.description,
            created_at: now,
            updated_at: now,
        };
        self.indicators
            .lock()
            .await
            .insert(indicator.indicator_id, indicator.clone());
        Ok(indicator)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        indicator: CustomIndicator,
    ) -> Result<CustomIndicator, CustomIndicatorRepositoryError> {
        self.record_transaction(transaction).await?;
        self.indicators
            .lock()
            .await
            .insert(indicator.indicator_id, indicator.clone());
        Ok(indicator)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        indicator_id: Uuid,
    ) -> Result<bool, CustomIndicatorRepositoryError> {
        self.record_transaction(transaction).await?;
        Ok(self.indicators.lock().await.remove(&indicator_id).is_some())
    }
}

impl FakeCustomIndicatorRepository {
    async fn record_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<(), CustomIndicatorRepositoryError> {
        let transaction_id = transaction
            .downcast_ref::<FakeTransaction>()
            .map(|transaction| transaction.id)
            .ok_or(CustomIndicatorRepositoryError::InvalidTransaction)?;
        self.transaction_ids.lock().await.push(transaction_id);
        Ok(())
    }
}
