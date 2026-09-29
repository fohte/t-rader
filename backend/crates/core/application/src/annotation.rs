use chrono::Utc;
use serde_json::{Map, Value, json};
use thiserror::Error;
use uuid::Uuid;

mod ports;

pub use ports::{
    Annotation, AnnotationRepository, AnnotationRepositoryError, ChangeAnnotationStatusCommand,
    CreateAnnotationCommand, DeleteAnnotationCommand, NewAnnotation, SharedAnnotationRepository,
    UpdateAnnotationCommand,
};

use crate::change_history::{
    Actor, ChangeHistoryError, ChangeHistoryRecord, Op, SharedChangeHistoryPort, TargetKind,
};
use crate::strategy_existence::{SharedStrategyExistence, StrategyExistenceError};
use crate::strategy_scope::StrategyScope;
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkError, UnitOfWorkTransaction};

const ALLOWED_STATUS: [&str; 3] = ["approved", "unread", "rejected"];
const ALLOWED_CREATED_BY_KIND: [&str; 2] = ["human", "llm"];

#[derive(Debug, Error)]
pub enum AnnotationUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("annotation {0} not found")]
    NotFound(Uuid),
    #[error("linked note {0} not found")]
    LinkedNoteNotFound(Uuid),
    #[error("annotation belongs to a different strategy scope")]
    ScopeMismatch,
    #[error(transparent)]
    Repository(#[from] AnnotationRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    ChangeHistory(#[from] ChangeHistoryError),
    #[error(transparent)]
    StrategyExistence(#[from] StrategyExistenceError),
}

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
        stale_ids.sort_unstable();
        let commented = self
            .repository
            .commented_annotation_ids_in_transaction(transaction, &stale_ids)
            .await?;
        for id in stale_ids {
            if commented.contains(&id) || !self.repository.delete(transaction, id).await? {
                continue;
            }
            self.record(transaction, actor, id, Op::Delete, json!({}), None)
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

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{DateTime, FixedOffset, Utc};
    use rstest::rstest;
    use serde_json::json;
    use tokio::sync::Mutex;

    use super::*;
    use crate::change_history::{FakeChangeHistory, FakeChangeHistoryEntry};
    use crate::strategy_existence::FakeStrategyExistence;
    use crate::unit_of_work::{FakeTransaction, FakeUnitOfWork};

    #[derive(Default)]
    struct FakeAnnotationRepository {
        annotations: Mutex<HashMap<Uuid, Annotation>>,
        commented: Mutex<HashSet<Uuid>>,
        note_strategy_ids: Mutex<HashMap<Uuid, Option<Uuid>>>,
        transaction_ids: Mutex<Vec<Uuid>>,
        update_calls: Mutex<usize>,
    }

    impl FakeAnnotationRepository {
        async fn insert_annotation(&self, annotation: Annotation) {
            self.annotations
                .lock()
                .await
                .insert(annotation.id, annotation);
        }

        async fn set_comment(&self, annotation_id: Uuid) {
            self.commented.lock().await.insert(annotation_id);
        }

        async fn set_note_strategy(&self, note_id: Uuid, strategy_id: Option<Uuid>) {
            self.note_strategy_ids
                .lock()
                .await
                .insert(note_id, strategy_id);
        }

        async fn record_transaction(
            &self,
            transaction: &UnitOfWorkTransaction,
        ) -> Result<(), AnnotationRepositoryError> {
            let transaction_id = transaction
                .downcast_ref::<FakeTransaction>()
                .map(|transaction| transaction.id)
                .ok_or(AnnotationRepositoryError::InvalidTransaction)?;
            self.transaction_ids.lock().await.push(transaction_id);
            Ok(())
        }
    }

    #[async_trait]
    impl AnnotationRepository for FakeAnnotationRepository {
        async fn find_by_id_in_transaction(
            &self,
            transaction: &UnitOfWorkTransaction,
            id: Uuid,
        ) -> Result<Option<Annotation>, AnnotationRepositoryError> {
            self.record_transaction(transaction).await?;
            Ok(self.annotations.lock().await.get(&id).cloned())
        }

        async fn insert(
            &self,
            transaction: &UnitOfWorkTransaction,
            annotation: NewAnnotation,
        ) -> Result<Annotation, AnnotationRepositoryError> {
            self.record_transaction(transaction).await?;
            let now = Utc::now().fixed_offset();
            let created = Annotation {
                id: annotation.id,
                strategy_id: annotation.strategy_id,
                target_symbol: annotation.target_symbol,
                target_kind: annotation.target_kind,
                timestamp: annotation.timestamp,
                price: annotation.price,
                text: annotation.text,
                status: annotation.status,
                linked_note_id: annotation.linked_note_id,
                created_by_kind: annotation.created_by_kind,
                created_at: now,
                updated_at: now,
                execution_step_id: annotation.execution_step_id,
                execution_task_id: annotation.execution_task_id,
            };
            self.annotations
                .lock()
                .await
                .insert(created.id, created.clone());
            Ok(created)
        }

        async fn update(
            &self,
            transaction: &UnitOfWorkTransaction,
            annotation: Annotation,
        ) -> Result<Annotation, AnnotationRepositoryError> {
            self.record_transaction(transaction).await?;
            *self.update_calls.lock().await += 1;
            self.annotations
                .lock()
                .await
                .insert(annotation.id, annotation.clone());
            Ok(annotation)
        }

        async fn delete(
            &self,
            transaction: &UnitOfWorkTransaction,
            id: Uuid,
        ) -> Result<bool, AnnotationRepositoryError> {
            self.record_transaction(transaction).await?;
            Ok(self.annotations.lock().await.remove(&id).is_some())
        }

        async fn find_stale_unread_in_transaction(
            &self,
            transaction: &UnitOfWorkTransaction,
            strategy_id: Uuid,
            execution_step_id: Uuid,
            current_execution_task_id: &str,
        ) -> Result<Vec<Uuid>, AnnotationRepositoryError> {
            self.record_transaction(transaction).await?;
            let mut stale: Vec<_> = self
                .annotations
                .lock()
                .await
                .values()
                .filter(|annotation| {
                    annotation.strategy_id == Some(strategy_id)
                        && annotation.execution_step_id == Some(execution_step_id)
                        && annotation.status == "unread"
                        && annotation.execution_task_id.as_deref()
                            != Some(current_execution_task_id)
                })
                .map(|annotation| annotation.id)
                .collect();
            stale.sort_unstable();
            Ok(stale)
        }

        async fn commented_annotation_ids_in_transaction(
            &self,
            transaction: &UnitOfWorkTransaction,
            annotation_ids: &[Uuid],
        ) -> Result<HashSet<Uuid>, AnnotationRepositoryError> {
            self.record_transaction(transaction).await?;
            let commented = self.commented.lock().await;
            Ok(annotation_ids
                .iter()
                .filter(|id| commented.contains(*id))
                .copied()
                .collect())
        }

        async fn note_strategy_id_in_transaction(
            &self,
            transaction: &UnitOfWorkTransaction,
            note_id: Uuid,
        ) -> Result<Option<Option<Uuid>>, AnnotationRepositoryError> {
            self.record_transaction(transaction).await?;
            Ok(self.note_strategy_ids.lock().await.get(&note_id).copied())
        }
    }

    fn build_use_cases() -> (
        AnnotationUseCases,
        Arc<FakeUnitOfWork>,
        Arc<FakeAnnotationRepository>,
        Arc<FakeStrategyExistence>,
        Arc<FakeChangeHistory>,
    ) {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeAnnotationRepository::default());
        let strategy_existence = Arc::new(FakeStrategyExistence::new());
        let change_history = Arc::new(FakeChangeHistory::new());
        let use_cases = AnnotationUseCases::new(
            unit_of_work.clone(),
            repository.clone(),
            strategy_existence.clone(),
            change_history.clone(),
        );
        (
            use_cases,
            unit_of_work,
            repository,
            strategy_existence,
            change_history,
        )
    }

    fn fixed_timestamp() -> DateTime<FixedOffset> {
        DateTime::<Utc>::UNIX_EPOCH.fixed_offset()
    }

    fn annotation(id: Uuid, strategy_id: Uuid, step_id: Uuid, task_id: &str) -> Annotation {
        Annotation {
            id,
            strategy_id: Some(strategy_id),
            target_symbol: "FICTIONAL-ASSET".into(),
            target_kind: "test-kind".into(),
            timestamp: fixed_timestamp(),
            price: None,
            text: "sample text".into(),
            status: "unread".into(),
            linked_note_id: None,
            created_by_kind: "llm".into(),
            created_at: fixed_timestamp(),
            updated_at: fixed_timestamp(),
            execution_step_id: Some(step_id),
            execution_task_id: Some(task_id.into()),
        }
    }

    fn create_command(strategy_id: Option<Uuid>) -> CreateAnnotationCommand {
        CreateAnnotationCommand {
            scope: None,
            actor: Actor::Llm { label: "analyst" },
            strategy_id,
            target_symbol: " FICTIONAL-ASSET ".into(),
            target_kind: " test-kind ".into(),
            timestamp: fixed_timestamp(),
            price: None,
            text: "sample text".into(),
            status: "unread".into(),
            linked_note_id: None,
            created_by_kind: "llm".into(),
            execution_step_id: None,
            execution_task_id: None,
        }
    }

    #[tokio::test]
    async fn create_replaces_only_uncommented_stale_annotations_and_records_history_in_one_transaction()
     {
        let (use_cases, unit_of_work, repository, strategy_existence, change_history) =
            build_use_cases();
        let strategy_id = Uuid::from_u128(1);
        let step_id = Uuid::from_u128(2);
        let removable_id = Uuid::from_u128(3);
        let commented_id = Uuid::from_u128(4);
        let current_attempt_id = Uuid::from_u128(5);
        let other_step_id = Uuid::from_u128(6);
        strategy_existence.insert_strategy(strategy_id).await;
        repository
            .insert_annotation(annotation(
                removable_id,
                strategy_id,
                step_id,
                "old-attempt",
            ))
            .await;
        repository
            .insert_annotation(annotation(
                commented_id,
                strategy_id,
                step_id,
                "old-attempt",
            ))
            .await;
        repository.set_comment(commented_id).await;
        repository
            .insert_annotation(annotation(
                current_attempt_id,
                strategy_id,
                step_id,
                "new-attempt",
            ))
            .await;
        repository
            .insert_annotation(annotation(
                other_step_id,
                strategy_id,
                Uuid::from_u128(7),
                "old-attempt",
            ))
            .await;
        let mut command = create_command(Some(strategy_id));
        command.execution_step_id = Some(step_id);
        command.execution_task_id = Some("new-attempt".into());

        let created = use_cases.create(command).await.expect("create succeeds");
        let transaction_id = unit_of_work.begun.lock().await[0];
        let mut remaining_ids: Vec<_> = repository
            .annotations
            .lock()
            .await
            .keys()
            .copied()
            .collect();
        remaining_ids.sort();
        let mut expected_remaining_ids =
            vec![commented_id, current_attempt_id, other_step_id, created.id];
        expected_remaining_ids.sort();
        let history: Vec<_> = change_history
            .entries
            .lock()
            .await
            .iter()
            .map(|entry: &FakeChangeHistoryEntry| {
                (
                    entry.transaction_id == transaction_id,
                    entry.record.actor,
                    entry.record.target_kind,
                    entry.record.target_id,
                    entry.record.op,
                    entry.record.diff.clone(),
                    entry.record.summary.clone(),
                )
            })
            .collect();
        let repository_transactions: Vec<_> = repository
            .transaction_ids
            .lock()
            .await
            .iter()
            .map(|id| *id == transaction_id)
            .collect();
        let strategy_transactions: Vec<_> = strategy_existence
            .transaction_ids()
            .await
            .iter()
            .map(|id| *id == transaction_id)
            .collect();

        assert_eq!(
            (
                remaining_ids,
                unit_of_work.begun.lock().await.len(),
                unit_of_work.committed.lock().await.clone(),
                repository_transactions,
                strategy_transactions,
                history,
            ),
            (
                expected_remaining_ids,
                1,
                vec![transaction_id],
                vec![true, true, true, true],
                vec![true],
                vec![
                    (
                        true,
                        Actor::Llm { label: "analyst" },
                        TargetKind::Annotation,
                        removable_id,
                        Op::Delete,
                        json!({}),
                        None,
                    ),
                    (
                        true,
                        Actor::Llm { label: "analyst" },
                        TargetKind::Annotation,
                        created.id,
                        Op::Create,
                        json!({
                            "strategy_id": strategy_id,
                            "target_symbol": "FICTIONAL-ASSET",
                        }),
                        None,
                    ),
                ],
            ),
        );
    }

    #[rstest]
    #[case::empty_text("text", "   ")]
    #[case::empty_symbol("target_symbol", "   ")]
    #[case::empty_target_kind("target_kind", "   ")]
    #[case::invalid_status("status", "pending")]
    #[case::invalid_created_by_kind("created_by_kind", "agent")]
    #[tokio::test]
    async fn create_rejects_invalid_fields_before_opening_transaction(
        #[case] field: &str,
        #[case] value: &str,
    ) {
        let (use_cases, unit_of_work, _, _, _) = build_use_cases();
        let mut command = create_command(None);
        match field {
            "text" => command.text = value.into(),
            "target_symbol" => command.target_symbol = value.into(),
            "target_kind" => command.target_kind = value.into(),
            "status" => command.status = value.into(),
            "created_by_kind" => command.created_by_kind = value.into(),
            _ => unreachable!(),
        }

        let result = use_cases.create(command).await;
        let validation_error = matches!(result, Err(AnnotationUseCaseError::Validation(_)));

        assert_eq!(
            (validation_error, unit_of_work.begun.lock().await.len()),
            (true, 0)
        );
    }

    #[tokio::test]
    async fn create_rejects_strategy_scope_mismatch() {
        let (use_cases, unit_of_work, _, _, _) = build_use_cases();
        let scope_id = Uuid::from_u128(10);
        let mut command = create_command(Some(Uuid::from_u128(11)));
        command.scope = Some(scope_id.into());

        let result = use_cases.create(command).await;

        assert_eq!(
            (
                matches!(result, Err(AnnotationUseCaseError::Validation(_))),
                unit_of_work.begun.lock().await.len(),
            ),
            (true, 0),
        );
    }

    #[tokio::test]
    async fn create_rejects_linked_note_from_another_strategy() {
        let (use_cases, unit_of_work, repository, strategy_existence, change_history) =
            build_use_cases();
        let strategy_id = Uuid::from_u128(20);
        let other_strategy_id = Uuid::from_u128(21);
        let note_id = Uuid::from_u128(22);
        strategy_existence.insert_strategy(strategy_id).await;
        repository
            .set_note_strategy(note_id, Some(other_strategy_id))
            .await;
        let mut command = create_command(Some(strategy_id));
        command.linked_note_id = Some(note_id);

        let result = use_cases.create(command).await;

        assert_eq!(
            (
                matches!(result, Err(AnnotationUseCaseError::Validation(_))),
                repository.annotations.lock().await.len(),
                change_history.entries.lock().await.len(),
                unit_of_work.committed.lock().await.len(),
            ),
            (true, 0, 0, 0),
        );
    }

    #[tokio::test]
    async fn change_status_does_not_update_or_record_history_when_status_is_unchanged() {
        let (use_cases, unit_of_work, repository, _, change_history) = build_use_cases();
        let id = Uuid::from_u128(30);
        let existing = Annotation {
            id,
            strategy_id: None,
            target_symbol: "FICTIONAL-ASSET".into(),
            target_kind: "test-kind".into(),
            timestamp: fixed_timestamp(),
            price: None,
            text: "sample text".into(),
            status: "approved".into(),
            linked_note_id: None,
            created_by_kind: "human".into(),
            created_at: fixed_timestamp(),
            updated_at: fixed_timestamp(),
            execution_step_id: None,
            execution_task_id: None,
        };
        repository.insert_annotation(existing.clone()).await;

        let returned = use_cases
            .change_status(ChangeAnnotationStatusCommand {
                scope: None,
                actor: Actor::Human,
                id,
                status: "approved".into(),
                label: None,
            })
            .await
            .expect("unchanged status succeeds");

        assert_eq!(
            (
                returned,
                change_history.entries.lock().await.len(),
                *repository.update_calls.lock().await,
                unit_of_work.committed.lock().await.len(),
            ),
            (existing, 0, 0, 0),
        );
    }
}
