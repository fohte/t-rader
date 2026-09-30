use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use rstest::rstest;
use serde_json::json;
use uuid::Uuid;

use super::fake::FakeAnnotationRepository;
use super::*;
use crate::change_history::{Actor, FakeChangeHistory, FakeChangeHistoryEntry, Op, TargetKind};
use crate::strategy_existence::FakeStrategyExistence;
use crate::unit_of_work::FakeUnitOfWork;

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
