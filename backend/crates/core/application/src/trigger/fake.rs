use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::repository::{TriggerRepository, TriggerRepositoryError};
use super::types::{NewTrigger, Trigger, TriggerKind};

#[derive(Default)]
pub struct FakeTriggerRepository {
    pub triggers: Mutex<HashMap<Uuid, Trigger>>,
    pub transaction_ids: Mutex<Vec<Uuid>>,
}

impl FakeTriggerRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert_trigger(&self, trigger: Trigger) {
        self.triggers
            .lock()
            .await
            .insert(trigger.trigger_id, trigger);
    }

    async fn record_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<(), TriggerRepositoryError> {
        let transaction_id = transaction
            .downcast_ref::<FakeTransaction>()
            .map(|transaction| transaction.id)
            .ok_or(TriggerRepositoryError::InvalidTransaction)?;
        self.transaction_ids.lock().await.push(transaction_id);
        Ok(())
    }
}

#[async_trait]
impl TriggerRepository for FakeTriggerRepository {
    async fn list_for_strategy(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
        kind: Option<TriggerKind>,
    ) -> Result<Vec<Trigger>, TriggerRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut triggers: Vec<_> = self
            .triggers
            .lock()
            .await
            .values()
            .filter(|trigger| {
                trigger.strategy_id == Some(strategy_id)
                    && kind.is_none_or(|kind| trigger.kind == kind)
            })
            .cloned()
            .collect();
        triggers.sort_by_key(|trigger| trigger.created_at);
        Ok(triggers)
    }

    async fn find_by_id(
        &self,
        trigger_id: Uuid,
    ) -> Result<Option<Trigger>, TriggerRepositoryError> {
        Ok(self.triggers.lock().await.get(&trigger_id).cloned())
    }

    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger_id: Uuid,
    ) -> Result<Option<Trigger>, TriggerRepositoryError> {
        self.record_transaction(transaction).await?;
        self.find_by_id(trigger_id).await
    }

    async fn find_enabled_hook_by_slug(
        &self,
        hook_slug: &str,
    ) -> Result<Option<Trigger>, TriggerRepositoryError> {
        Ok(self
            .triggers
            .lock()
            .await
            .values()
            .find(|trigger| {
                trigger.kind == TriggerKind::Hook
                    && trigger.enabled
                    && trigger.hook_slug.as_deref() == Some(hook_slug)
            })
            .cloned())
    }

    async fn list_enabled_cron(&self) -> Result<Vec<Trigger>, TriggerRepositoryError> {
        let mut triggers: Vec<_> = self
            .triggers
            .lock()
            .await
            .values()
            .filter(|trigger| trigger.kind == TriggerKind::Cron && trigger.enabled)
            .cloned()
            .collect();
        triggers.sort_by_key(|trigger| trigger.created_at);
        Ok(triggers)
    }

    async fn create(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger: NewTrigger,
    ) -> Result<Trigger, TriggerRepositoryError> {
        self.record_transaction(transaction).await?;
        let now = Utc::now().fixed_offset();
        let trigger = Trigger {
            trigger_id: trigger.trigger_id,
            strategy_id: Some(trigger.strategy_id),
            kind: trigger.kind,
            schedule: trigger.schedule,
            hook_slug: trigger.hook_slug,
            event_match: trigger.event_match,
            prompt_template: trigger.prompt_template,
            enabled: trigger.enabled,
            last_fired_at: None,
            created_at: now,
            updated_at: now,
        };
        self.insert_trigger(trigger.clone()).await;
        Ok(trigger)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger: Trigger,
    ) -> Result<Trigger, TriggerRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut triggers = self.triggers.lock().await;
        match triggers.entry(trigger.trigger_id) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.insert(trigger.clone());
                Ok(trigger)
            }
            std::collections::hash_map::Entry::Vacant(_) => Err(TriggerRepositoryError::Database(
                PersistenceError::RecordNotUpdated(format!(
                    "trigger {} not found",
                    trigger.trigger_id
                )),
            )),
        }
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger_id: Uuid,
    ) -> Result<bool, TriggerRepositoryError> {
        self.record_transaction(transaction).await?;
        Ok(self.triggers.lock().await.remove(&trigger_id).is_some())
    }

    async fn mark_fired(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger_id: Uuid,
        fired_at: DateTime<FixedOffset>,
    ) -> Result<bool, TriggerRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut triggers = self.triggers.lock().await;
        let Some(trigger) = triggers.get_mut(&trigger_id) else {
            return Ok(false);
        };
        trigger.last_fired_at = Some(fired_at);
        trigger.updated_at = fired_at;
        Ok(true)
    }
}
