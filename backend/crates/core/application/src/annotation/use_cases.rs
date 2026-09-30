use chrono::Utc;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::error::AnnotationUseCaseError;
use super::ports::{
    Annotation, ChangeAnnotationStatusCommand, CreateAnnotationCommand, DeleteAnnotationCommand,
    NewAnnotation, SharedAnnotationRepository, UpdateAnnotationCommand,
};
use crate::change_history::{Actor, ChangeHistoryRecord, Op, SharedChangeHistoryPort, TargetKind};
use crate::strategy_existence::SharedStrategyExistence;
use crate::strategy_scope::StrategyScope;
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction};

const ALLOWED_STATUS: [&str; 3] = ["approved", "unread", "rejected"];
const ALLOWED_CREATED_BY_KIND: [&str; 2] = ["human", "llm"];

#[derive(Clone)]
pub struct AnnotationUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedAnnotationRepository,
    strategy_existence: SharedStrategyExistence,
    change_history: SharedChangeHistoryPort,
}

impl AnnotationUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedAnnotationRepository,
        strategy_existence: SharedStrategyExistence,
        change_history: SharedChangeHistoryPort,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            strategy_existence,
            change_history,
        }
    }

    pub async fn create(
        &self,
        command: CreateAnnotationCommand,
    ) -> Result<Annotation, AnnotationUseCaseError> {
        let target_symbol = non_empty_trimmed(command.target_symbol, "target_symbol")?;
        let target_kind = non_empty_trimmed(command.target_kind, "target_kind")?;
        validate_non_empty(&command.text, "text")?;
        validate_status(&command.status)?;
        validate_created_by_kind(&command.created_by_kind)?;
        validate_scope_strategy(command.scope, command.strategy_id)?;

        let transaction = self.unit_of_work.begin().await?;
        if let Some(strategy_id) = command.strategy_id {
            self.ensure_strategy_exists(&transaction, strategy_id)
                .await?;
        }
        if let Some(note_id) = command.linked_note_id {
            self.ensure_linked_note_scope(&transaction, note_id, command.strategy_id)
                .await?;
        }

        if let (Some(strategy_id), Some(step_id), Some(task_id)) = (
            command.strategy_id,
            command.execution_step_id,
            command.execution_task_id.as_deref(),
        ) {
            self.replace_stale_annotations(
                &transaction,
                strategy_id,
                step_id,
                task_id,
                command.actor,
            )
            .await?;
        }

        let id = Uuid::new_v4();
        let created = self
            .repository
            .insert(
                &transaction,
                NewAnnotation {
                    id,
                    strategy_id: command.strategy_id,
                    target_symbol: target_symbol.clone(),
                    target_kind,
                    timestamp: command.timestamp,
                    price: command.price,
                    text: command.text,
                    status: command.status,
                    linked_note_id: command.linked_note_id,
                    created_by_kind: command.created_by_kind,
                    execution_step_id: command.execution_step_id,
                    execution_task_id: command.execution_task_id,
                },
            )
            .await?;
        self.record(
            &transaction,
            command.actor,
            id,
            Op::Create,
            json!({
                "strategy_id": command.strategy_id,
                "target_symbol": target_symbol,
            }),
            None,
        )
        .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(created)
    }

    pub async fn update(
        &self,
        command: UpdateAnnotationCommand,
    ) -> Result<Annotation, AnnotationUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let current = self
            .repository
            .find_by_id_in_transaction(&transaction, command.id)
            .await?
            .ok_or(AnnotationUseCaseError::NotFound(command.id))?;
        ensure_scope_owns(command.scope, current.strategy_id)?;

        let mut next = current.clone();
        let mut diff = Map::new();
        if let Some(value) = command.target_symbol {
            let value = non_empty_trimmed(value, "target_symbol")?;
            diff.insert(
                "target_symbol".into(),
                json!({ "from": current.target_symbol, "to": value }),
            );
            next.target_symbol = value;
        }
        if let Some(value) = command.target_kind {
            let value = non_empty_trimmed(value, "target_kind")?;
            diff.insert(
                "target_kind".into(),
                json!({ "from": current.target_kind, "to": value }),
            );
            next.target_kind = value;
        }
        if let Some(value) = command.timestamp {
            diff.insert(
                "timestamp".into(),
                json!({ "from": current.timestamp, "to": value }),
            );
            next.timestamp = value;
        }
        if let Some(value) = command.price {
            diff.insert(
                "price".into(),
                json!({ "from": current.price, "to": value }),
            );
            next.price = Some(value);
        }
        if let Some(value) = command.text {
            validate_non_empty(&value, "text")?;
            diff.insert(
                "text".into(),
                json!({ "len_from": current.text.len(), "len_to": value.len() }),
            );
            next.text = value;
        }
        if let Some(value) = command.linked_note_id {
            self.ensure_linked_note_scope(&transaction, value, current.strategy_id)
                .await?;
            diff.insert(
                "linked_note_id".into(),
                json!({ "from": current.linked_note_id, "to": value }),
            );
            next.linked_note_id = Some(value);
        }
        next.updated_at = Utc::now().fixed_offset();

        let updated = self.repository.update(&transaction, next).await?;
        if !diff.is_empty() {
            self.record(
                &transaction,
                command.actor,
                command.id,
                Op::Update,
                Value::Object(diff),
                None,
            )
            .await?;
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    pub async fn change_status(
        &self,
        command: ChangeAnnotationStatusCommand,
    ) -> Result<Annotation, AnnotationUseCaseError> {
        validate_status(&command.status)?;
        let transaction = self.unit_of_work.begin().await?;
        let current = self
            .repository
            .find_by_id_in_transaction(&transaction, command.id)
            .await?
            .ok_or(AnnotationUseCaseError::NotFound(command.id))?;
        ensure_scope_owns(command.scope, current.strategy_id)?;
        if current.status == command.status {
            return Ok(current);
        }

        let mut updated = current.clone();
        updated.status = command.status.clone();
        updated.updated_at = Utc::now().fixed_offset();
        let updated = self.repository.update(&transaction, updated).await?;
        self.record(
            &transaction,
            command.actor,
            command.id,
            Op::StatusChange,
            json!({
                "from": current.status,
                "to": command.status,
                "label": command.label,
            }),
            command.label,
        )
        .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    pub async fn delete(
        &self,
        command: DeleteAnnotationCommand,
    ) -> Result<(), AnnotationUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let current = self
            .repository
            .find_by_id_in_transaction(&transaction, command.id)
            .await?
            .ok_or(AnnotationUseCaseError::NotFound(command.id))?;
        ensure_scope_owns(command.scope, current.strategy_id)?;
        if !self.repository.delete(&transaction, command.id).await? {
            return Err(AnnotationUseCaseError::NotFound(command.id));
        }
        self.record(
            &transaction,
            command.actor,
            command.id,
            Op::Delete,
            json!({}),
            None,
        )
        .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }

    async fn ensure_strategy_exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
    ) -> Result<(), AnnotationUseCaseError> {
        if !self
            .strategy_existence
            .exists(transaction, strategy_id)
            .await?
        {
            return Err(AnnotationUseCaseError::Validation(format!(
                "strategy {strategy_id} does not exist"
            )));
        }
        Ok(())
    }

    async fn ensure_linked_note_scope(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
        strategy_id: Option<Uuid>,
    ) -> Result<(), AnnotationUseCaseError> {
        let note_strategy_id = self
            .repository
            .note_strategy_id_in_transaction(transaction, note_id)
            .await?
            .ok_or(AnnotationUseCaseError::LinkedNoteNotFound(note_id))?;
        if note_strategy_id != strategy_id {
            return Err(AnnotationUseCaseError::Validation(
                "linked note belongs to a different strategy".into(),
            ));
        }
        Ok(())
    }

    async fn replace_stale_annotations(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
        execution_step_id: Uuid,
        current_execution_task_id: &str,
        actor: Actor,
    ) -> Result<(), AnnotationUseCaseError> {
        // 同じ step の実行が別の task に切り替わったとき、前 task の未レビュー annotation を置き換える。
        // 1 step から複数件作成できるため、一意制約ではなく古い未レビュー分を削除する。
        let mut stale_ids = self
            .repository
            .find_stale_unread_in_transaction(
                transaction,
                strategy_id,
                execution_step_id,
                current_execution_task_id,
            )
            .await?;
        if stale_ids.is_empty() {
            return Ok(());
        }
        stale_ids.sort();
        let commented = self
            .repository
            .commented_annotation_ids_in_transaction(transaction, &stale_ids)
            .await?;
        for annotation_id in stale_ids {
            // comment.target_id は FK を持たず、削除すると comment が孤児化するため、コメント付きは残す。
            if commented.contains(&annotation_id)
                || !self.repository.delete(transaction, annotation_id).await?
            {
                continue;
            }
            self.record(
                transaction,
                actor,
                annotation_id,
                Op::Delete,
                json!({}),
                None,
            )
            .await?;
        }
        Ok(())
    }

    async fn record(
        &self,
        transaction: &UnitOfWorkTransaction,
        actor: Actor,
        target_id: Uuid,
        op: Op,
        diff: Value,
        summary: Option<String>,
    ) -> Result<(), AnnotationUseCaseError> {
        self.change_history
            .record(
                transaction,
                ChangeHistoryRecord {
                    actor,
                    target_kind: TargetKind::Annotation,
                    target_id,
                    op,
                    diff,
                    summary,
                },
            )
            .await?;
        Ok(())
    }
}

fn validate_scope_strategy(
    scope: Option<StrategyScope>,
    strategy_id: Option<Uuid>,
) -> Result<(), AnnotationUseCaseError> {
    if scope.is_some_and(|scope| strategy_id != Some(scope.id())) {
        return Err(AnnotationUseCaseError::Validation(
            "strategy_id must match the strategy scope".into(),
        ));
    }
    Ok(())
}

fn ensure_scope_owns(
    scope: Option<StrategyScope>,
    strategy_id: Option<Uuid>,
) -> Result<(), AnnotationUseCaseError> {
    if scope.is_some_and(|scope| strategy_id != Some(scope.id())) {
        return Err(AnnotationUseCaseError::ScopeMismatch);
    }
    Ok(())
}

fn non_empty_trimmed(value: String, name: &str) -> Result<String, AnnotationUseCaseError> {
    let value = value.trim().to_string();
    validate_non_empty(&value, name)?;
    Ok(value)
}

fn validate_non_empty(value: &str, name: &str) -> Result<(), AnnotationUseCaseError> {
    if value.trim().is_empty() {
        return Err(AnnotationUseCaseError::Validation(format!(
            "{name} must not be empty"
        )));
    }
    Ok(())
}

fn validate_status(status: &str) -> Result<(), AnnotationUseCaseError> {
    if ALLOWED_STATUS.contains(&status) {
        Ok(())
    } else {
        Err(AnnotationUseCaseError::Validation(format!(
            "invalid status: {status}"
        )))
    }
}

fn validate_created_by_kind(kind: &str) -> Result<(), AnnotationUseCaseError> {
    if ALLOWED_CREATED_BY_KIND.contains(&kind) {
        Ok(())
    } else {
        Err(AnnotationUseCaseError::Validation(format!(
            "invalid created_by_kind: {kind}"
        )))
    }
}
