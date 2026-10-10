use chrono::Utc;
use core_domain::note_price_reference::PriceReferenceField;
use rust_decimal::Decimal;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::error::AnnotationUseCaseError;
use super::ports::{
    Annotation, AnnotationCreateResult, AnnotationPriceInput, ChangeAnnotationStatusCommand,
    CreateAnnotationCommand, DeleteAnnotationCommand, NewAnnotation, SharedAnnotationRepository,
    UpdateAnnotationCommand,
};
use crate::change_history::{Actor, ChangeHistoryRecord, Op, SharedChangeHistoryPort, TargetKind};
use crate::strategy_task_step_evidence::{
    SharedStrategyTaskStepEvidenceRepository, StrategyTaskStepEvidence,
};
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction};

const ALLOWED_STATUS: [&str; 3] = ["approved", "unread", "rejected"];
const ALLOWED_CREATED_BY_KIND: [&str; 2] = ["human", "llm"];
const QUERY_DATA_RANGE_START_WARNING: &str = "timestamp_start がこの実行の query_data の取得開始日と一致しています。観測期間ではなく、アノテーション自身が語る期間の開始日か確認してください。";

#[derive(Clone)]
pub struct AnnotationUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedAnnotationRepository,
    change_history: SharedChangeHistoryPort,
    strategy_task_step_evidence: SharedStrategyTaskStepEvidenceRepository,
}

impl AnnotationUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedAnnotationRepository,
        change_history: SharedChangeHistoryPort,
        strategy_task_step_evidence: SharedStrategyTaskStepEvidenceRepository,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            change_history,
            strategy_task_step_evidence,
        }
    }

    pub async fn create(
        &self,
        command: CreateAnnotationCommand,
    ) -> Result<Annotation, AnnotationUseCaseError> {
        self.create_with_warnings(command)
            .await
            .map(|result| result.annotation)
    }

    pub async fn create_with_warnings(
        &self,
        command: CreateAnnotationCommand,
    ) -> Result<AnnotationCreateResult, AnnotationUseCaseError> {
        validate_timestamp_range(command.timestamp_start, command.timestamp)?;
        let price_input = validate_price_input(command.price, command.execution_step_id)?;
        let target_symbol = non_empty_trimmed(command.target_symbol, "target_symbol")?;
        let target_kind = non_empty_trimmed(command.target_kind, "target_kind")?;
        validate_non_empty(&command.text, "text")?;
        validate_status(&command.status)?;
        validate_created_by_kind(&command.created_by_kind)?;
        let transaction = self.unit_of_work.begin().await?;
        let needs_query_data = command.timestamp_start.is_some()
            || matches!(&price_input, ValidatedAnnotationPriceInput::Field(_));
        let query_data = match command.execution_step_id {
            Some(execution_step_id) if needs_query_data => {
                self.strategy_task_step_evidence
                    .find_query_data(&transaction, execution_step_id, &target_symbol)
                    .await?
            }
            _ => Vec::new(),
        };
        let warnings = Self::timestamp_start_warnings(&query_data, command.timestamp_start);
        if let Some(note_id) = command.linked_note_id {
            self.ensure_linked_note_exists(&transaction, note_id)
                .await?;
        }

        let price = match price_input {
            ValidatedAnnotationPriceInput::None => None,
            ValidatedAnnotationPriceInput::Value(value) => Some(value),
            ValidatedAnnotationPriceInput::Field(field) => Some(Self::resolve_price_field(
                &query_data,
                command.timestamp.with_timezone(&Utc).date_naive(),
                field,
            )?),
        };

        if let (Some(step_id), Some(task_id)) = (
            command.execution_step_id,
            command.execution_task_id.as_deref(),
        ) {
            self.replace_stale_annotations(&transaction, step_id, task_id, command.actor)
                .await?;
        }

        let id = Uuid::new_v4();
        let created = self
            .repository
            .insert(
                &transaction,
                NewAnnotation {
                    id,
                    target_symbol: target_symbol.clone(),
                    target_kind,
                    timestamp: command.timestamp,
                    timestamp_start: command.timestamp_start,
                    price,
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
                "target_symbol": target_symbol,
            }),
            None,
        )
        .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(AnnotationCreateResult {
            annotation: created,
            warnings,
        })
    }

    fn timestamp_start_warnings(
        evidence: &[StrategyTaskStepEvidence],
        timestamp_start: Option<chrono::DateTime<chrono::FixedOffset>>,
    ) -> Vec<String> {
        let Some(timestamp_start) = timestamp_start else {
            return Vec::new();
        };
        let timestamp_start_date = timestamp_start.with_timezone(&Utc).date_naive();

        if evidence
            .iter()
            .any(|snapshot| snapshot.query_data_range_start() == Some(timestamp_start_date))
        {
            vec![QUERY_DATA_RANGE_START_WARNING.into()]
        } else {
            Vec::new()
        }
    }

    fn resolve_price_field(
        evidence: &[StrategyTaskStepEvidence],
        date: chrono::NaiveDate,
        field: PriceReferenceField,
    ) -> Result<Decimal, AnnotationUseCaseError> {
        let value = evidence
            .iter()
            .rev()
            .find_map(|snapshot| snapshot.query_data_bar_value(date, field))
            .and_then(|value| value.as_f64())
            .and_then(|value| Decimal::try_from(value).ok())
            .ok_or_else(|| {
                AnnotationUseCaseError::Validation(
                    "指定された価格項目を実行ステップの query_data から解決できません".into(),
                )
            })?;
        Ok(value)
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
            self.ensure_linked_note_exists(&transaction, value).await?;
            diff.insert(
                "linked_note_id".into(),
                json!({ "from": current.linked_note_id, "to": value }),
            );
            next.linked_note_id = Some(value);
        }
        validate_timestamp_range(next.timestamp_start, next.timestamp)?;
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
        self.repository
            .find_by_id_in_transaction(&transaction, command.id)
            .await?
            .ok_or(AnnotationUseCaseError::NotFound(command.id))?;
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

    async fn ensure_linked_note_exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<(), AnnotationUseCaseError> {
        let exists = self
            .repository
            .note_exists_in_transaction(transaction, note_id)
            .await?;
        if exists {
            Ok(())
        } else {
            Err(AnnotationUseCaseError::LinkedNoteNotFound(note_id))
        }
    }

    async fn replace_stale_annotations(
        &self,
        transaction: &UnitOfWorkTransaction,
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

enum ValidatedAnnotationPriceInput {
    None,
    Value(Decimal),
    Field(PriceReferenceField),
}

fn validate_timestamp_range(
    timestamp_start: Option<chrono::DateTime<chrono::FixedOffset>>,
    timestamp: chrono::DateTime<chrono::FixedOffset>,
) -> Result<(), AnnotationUseCaseError> {
    if timestamp_start.is_some_and(|start| start > timestamp) {
        return Err(AnnotationUseCaseError::Validation(
            "timestamp_start must not be after timestamp".into(),
        ));
    }
    Ok(())
}

fn validate_price_input(
    price: Option<AnnotationPriceInput>,
    execution_step_id: Option<Uuid>,
) -> Result<ValidatedAnnotationPriceInput, AnnotationUseCaseError> {
    match price {
        None => Ok(ValidatedAnnotationPriceInput::None),
        Some(AnnotationPriceInput::Value(value)) => Ok(ValidatedAnnotationPriceInput::Value(value)),
        Some(AnnotationPriceInput::Field(PriceReferenceField::Volume)) => {
            Err(AnnotationUseCaseError::Validation(
                "価格項目には `open`, `high`, `low`, `close` のいずれかを指定してください".into(),
            ))
        }
        Some(AnnotationPriceInput::Field(field)) => execution_step_id
            .map(|_| ValidatedAnnotationPriceInput::Field(field))
            .ok_or_else(|| {
                AnnotationUseCaseError::Validation(
                    "価格項目の解決には実行ステップの query_data が必要です".into(),
                )
            }),
    }
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
