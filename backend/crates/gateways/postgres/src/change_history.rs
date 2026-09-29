use async_trait::async_trait;
use core_application::change_history::{
    ChangeHistoryError, ChangeHistoryPort, ChangeHistoryRecord,
};
use sea_orm::ActiveValue::Set;
use sea_orm::EntityTrait;
use uuid::Uuid;

use crate::entities::change_history;
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref;

pub struct PostgresChangeHistory;

#[async_trait]
impl ChangeHistoryPort for PostgresChangeHistory {
    async fn record(
        &self,
        transaction: &core_application::unit_of_work::UnitOfWorkTransaction,
        record: ChangeHistoryRecord,
    ) -> Result<(), ChangeHistoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(ChangeHistoryError::InvalidTransaction)?;
        let model = change_history::ActiveModel {
            id: Set(Uuid::new_v4()),
            target_kind: Set(record.target_kind.as_str().to_string()),
            target_id: Set(record.target_id),
            actor_kind: Set(record.actor.kind().to_string()),
            actor_label: Set(record.actor.label().to_string()),
            op: Set(record.op.as_str().to_string()),
            diff_json: Set(record.diff),
            summary: Set(record.summary),
            created_at: sea_orm::ActiveValue::NotSet,
        };
        change_history::Entity::insert(model)
            .exec_without_returning(transaction)
            .await
            .map(|_| ())
            .map_err(|error| ChangeHistoryError::Database(persistence_error(error)))
    }
}
