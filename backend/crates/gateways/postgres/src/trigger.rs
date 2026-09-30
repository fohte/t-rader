use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use core_application::trigger::{
    NewTrigger, Trigger, TriggerKind, TriggerRepository, TriggerRepositoryError,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::trigger;
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

#[derive(Clone)]
pub struct PostgresTriggerRepository {
    db: DatabaseHandle,
}

impl PostgresTriggerRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TriggerRepository for PostgresTriggerRepository {
    async fn list_for_strategy(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
        kind: Option<TriggerKind>,
    ) -> Result<Vec<Trigger>, TriggerRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let mut query = trigger::Entity::find()
            .filter(trigger::Column::StrategyId.eq(strategy_id))
            .order_by_asc(trigger::Column::CreatedAt);
        if let Some(kind) = kind {
            query = query.filter(trigger::Column::Kind.eq(kind.as_str()));
        }
        query
            .all(transaction)
            .await
            .map_err(repository_error)?
            .into_iter()
            .map(to_domain)
            .collect()
    }

    async fn find_by_id(
        &self,
        trigger_id: Uuid,
    ) -> Result<Option<Trigger>, TriggerRepositoryError> {
        trigger::Entity::find_by_id(trigger_id)
            .one(&self.db)
            .await
            .map_err(repository_error)?
            .map(to_domain)
            .transpose()
    }

    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger_id: Uuid,
    ) -> Result<Option<Trigger>, TriggerRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        trigger::Entity::find_by_id(trigger_id)
            .one(transaction)
            .await
            .map_err(repository_error)?
            .map(to_domain)
            .transpose()
    }

    async fn find_enabled_hook_by_slug(
        &self,
        hook_slug: &str,
    ) -> Result<Option<Trigger>, TriggerRepositoryError> {
        trigger::Entity::find()
            .filter(trigger::Column::Kind.eq(TriggerKind::Hook.as_str()))
            .filter(trigger::Column::HookSlug.eq(hook_slug))
            .filter(trigger::Column::Enabled.eq(true))
            .one(&self.db)
            .await
            .map_err(repository_error)?
            .map(to_domain)
            .transpose()
    }

    async fn list_enabled_cron(&self) -> Result<Vec<Trigger>, TriggerRepositoryError> {
        trigger::Entity::find()
            .filter(trigger::Column::Kind.eq(TriggerKind::Cron.as_str()))
            .filter(trigger::Column::Enabled.eq(true))
            .all(&self.db)
            .await
            .map_err(repository_error)?
            .into_iter()
            .map(to_domain)
            .collect()
    }

    async fn create(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger: NewTrigger,
    ) -> Result<Trigger, TriggerRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = trigger::ActiveModel {
            trigger_id: Set(trigger.trigger_id),
            strategy_id: Set(Some(trigger.strategy_id)),
            kind: Set(trigger.kind.as_str().to_string()),
            schedule: Set(trigger.schedule),
            hook_slug: Set(trigger.hook_slug),
            event_match: Set(trigger.event_match),
            prompt_template: Set(trigger.prompt_template),
            enabled: Set(trigger.enabled),
            last_fired_at: sea_orm::ActiveValue::NotSet,
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        };
        trigger::Entity::insert(model)
            .exec_with_returning(transaction)
            .await
            .map_err(repository_error)
            .and_then(to_domain)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger: Trigger,
    ) -> Result<Trigger, TriggerRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let model = trigger::ActiveModel {
            trigger_id: sea_orm::ActiveValue::Unchanged(trigger.trigger_id),
            strategy_id: Set(trigger.strategy_id),
            kind: Set(trigger.kind.as_str().to_string()),
            schedule: Set(trigger.schedule),
            hook_slug: Set(trigger.hook_slug),
            event_match: Set(trigger.event_match),
            prompt_template: Set(trigger.prompt_template),
            enabled: Set(trigger.enabled),
            last_fired_at: sea_orm::ActiveValue::Unchanged(trigger.last_fired_at),
            created_at: sea_orm::ActiveValue::Unchanged(trigger.created_at),
            updated_at: Set(trigger.updated_at),
        };
        model
            .update(transaction)
            .await
            .map_err(repository_error)
            .and_then(to_domain)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger_id: Uuid,
    ) -> Result<bool, TriggerRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        trigger::Entity::delete_by_id(trigger_id)
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }

    async fn mark_fired(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger_id: Uuid,
        fired_at: DateTime<FixedOffset>,
    ) -> Result<bool, TriggerRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let Some(model) = trigger::Entity::find_by_id(trigger_id)
            .one(transaction)
            .await
            .map_err(repository_error)?
        else {
            return Ok(false);
        };
        let mut active = model.into_active_model();
        active.last_fired_at = Set(Some(fired_at));
        active.updated_at = Set(fired_at);
        active
            .update(transaction)
            .await
            .map(|_| true)
            .map_err(repository_error)
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, TriggerRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(TriggerRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> TriggerRepositoryError {
    TriggerRepositoryError::Database(persistence_error(error))
}

fn to_domain(model: trigger::Model) -> Result<Trigger, TriggerRepositoryError> {
    let kind = TriggerKind::try_from(model.kind.as_str())
        .map_err(|_| TriggerRepositoryError::InvalidKind(model.kind.clone()))?;
    Ok(Trigger {
        trigger_id: model.trigger_id,
        strategy_id: model.strategy_id,
        kind,
        schedule: model.schedule,
        hook_slug: model.hook_slug,
        event_match: model.event_match,
        prompt_template: model.prompt_template,
        enabled: model.enabled,
        last_fired_at: model.last_fired_at,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}
