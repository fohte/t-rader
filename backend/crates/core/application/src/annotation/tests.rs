use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use rstest::rstest;
use serde_json::json;
use tokio::sync::Mutex;
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

struct FakeAnnotationReadQuery {
    annotation: Annotation,
    listed_queries: Mutex<Vec<AnnotationListQuery>>,
}

#[async_trait::async_trait]
impl AnnotationReadQuery for FakeAnnotationReadQuery {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Annotation>, AnnotationReadQueryError> {
        Ok((self.annotation.id == id).then(|| self.annotation.clone()))
    }

    async fn list(
        &self,
        query: AnnotationListQuery,
    ) -> Result<Vec<Annotation>, AnnotationReadQueryError> {
        self.listed_queries.lock().await.push(query);
        Ok(vec![self.annotation.clone()])
    }

    async fn list_recent(
        &self,
        _strategy_id: Uuid,
        _limit: u64,
    ) -> Result<Vec<RecentAnnotation>, AnnotationReadQueryError> {
        Ok(Vec::new())
    }
}

#[rstest]
#[case::unassigned(None)]
#[case::another_strategy(Some(Uuid::from_u128(49)))]
#[tokio::test]
async fn get_annotation_accepts_annotations_with_any_strategy_owner(
    #[case] strategy_id: Option<Uuid>,
) {
    let mut target = annotation(
        Uuid::from_u128(40),
        Uuid::from_u128(41),
        Uuid::from_u128(42),
        "task",
    );
    target.strategy_id = strategy_id;
    let query = Arc::new(FakeAnnotationReadQuery {
        annotation: target.clone(),
        listed_queries: Mutex::new(Vec::new()),
    });
    let use_cases = AnnotationReadUseCases::new(query);

    let result = use_cases
        .get_annotation(target.id)
        .await
        .expect("an annotation is readable from a different strategy scope");

    assert_eq!(result, target);
}

#[tokio::test]
async fn list_annotations_preserves_the_query_strategy_filter() {
    let annotation = annotation(
        Uuid::from_u128(44),
        Uuid::from_u128(45),
        Uuid::from_u128(46),
        "task",
    );
    let query = Arc::new(FakeAnnotationReadQuery {
        annotation,
        listed_queries: Mutex::new(Vec::new()),
    });
    let use_cases = AnnotationReadUseCases::new(query.clone());
    let requested_query = AnnotationListQuery {
        strategy_id: Some(Uuid::from_u128(47)),
        target_symbol: Some("FICTIONAL-ASSET".into()),
        limit: Some(5),
    };

    use_cases
        .list_annotations(requested_query.clone())
        .await
        .expect("listing succeeds");

    assert_eq!(
        query.listed_queries.lock().await.clone(),
        vec![requested_query],
    );
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

#[rstest]
#[case::another_strategy(Some(Uuid::from_u128(21)))]
#[case::no_strategy(None)]
#[tokio::test]
async fn create_allows_linked_note_from_any_strategy(#[case] note_strategy_id: Option<Uuid>) {
    let (use_cases, unit_of_work, repository, strategy_existence, change_history) =
        build_use_cases();
    let strategy_id = Uuid::from_u128(20);
    let note_id = Uuid::from_u128(22);
    strategy_existence.insert_strategy(strategy_id).await;
    repository
        .set_note_strategy(note_id, note_strategy_id)
        .await;
    let mut command = create_command(Some(strategy_id));
    command.linked_note_id = Some(note_id);

    let created = use_cases
        .create(command)
        .await
        .expect("linked note from any strategy is accepted");

    assert_eq!(
        (
            created.strategy_id,
            created.linked_note_id,
            repository.annotations.lock().await.len(),
            change_history.entries.lock().await.len(),
            unit_of_work.committed.lock().await.len(),
        ),
        (Some(strategy_id), Some(note_id), 1, 1, 1),
    );
}

#[tokio::test]
async fn update_allows_annotation_from_another_strategy() {
    let (use_cases, _, repository, _, _) = build_use_cases();
    let id = Uuid::from_u128(30);
    let owner_strategy_id = Uuid::from_u128(31);
    repository
        .insert_annotation(annotation(
            id,
            owner_strategy_id,
            Uuid::from_u128(33),
            "task",
        ))
        .await;

    let updated = use_cases
        .update(UpdateAnnotationCommand {
            actor: Actor::Llm { label: "analyst" },
            id,
            target_symbol: None,
            target_kind: None,
            timestamp: None,
            price: None,
            text: Some("updated text".into()),
            linked_note_id: None,
        })
        .await
        .expect("annotation from another strategy is updateable");

    assert_eq!(
        (updated.id, updated.strategy_id, updated.text),
        (id, Some(owner_strategy_id), "updated text".into()),
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
