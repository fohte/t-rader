use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use core_domain::note_price_reference::PriceReferenceField;
use rstest::rstest;
use rust_decimal::Decimal;
use serde_json::json;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::fake::FakeAnnotationRepository;
use super::*;
use crate::change_history::{Actor, FakeChangeHistory, FakeChangeHistoryEntry, Op, TargetKind};
use crate::strategy_task_step_evidence::{
    FakeStrategyTaskStepEvidenceRepository, StrategyTaskStepEvidence,
};
use crate::unit_of_work::FakeUnitOfWork;

fn build_use_cases() -> (
    AnnotationUseCases,
    Arc<FakeUnitOfWork>,
    Arc<FakeAnnotationRepository>,
    Arc<FakeChangeHistory>,
) {
    build_use_cases_with_evidence(Arc::new(FakeStrategyTaskStepEvidenceRepository::default()))
}

fn build_use_cases_with_evidence(
    strategy_task_step_evidence: Arc<FakeStrategyTaskStepEvidenceRepository>,
) -> (
    AnnotationUseCases,
    Arc<FakeUnitOfWork>,
    Arc<FakeAnnotationRepository>,
    Arc<FakeChangeHistory>,
) {
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let repository = Arc::new(FakeAnnotationRepository::default());
    let change_history = Arc::new(FakeChangeHistory::new());
    let use_cases = AnnotationUseCases::new(
        unit_of_work.clone(),
        repository.clone(),
        change_history.clone(),
        strategy_task_step_evidence,
    );
    (use_cases, unit_of_work, repository, change_history)
}

fn fixed_timestamp() -> DateTime<FixedOffset> {
    DateTime::<Utc>::UNIX_EPOCH.fixed_offset()
}

fn annotation(id: Uuid, step_id: Uuid, task_id: &str) -> Annotation {
    Annotation {
        id,
        target_symbol: "FICTIONAL-ASSET".into(),
        target_kind: "test-kind".into(),
        timestamp: fixed_timestamp(),
        timestamp_start: None,
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

fn query_data_evidence(step_id: Uuid, source_ref: &str) -> StrategyTaskStepEvidence {
    let timestamp = fixed_timestamp();
    StrategyTaskStepEvidence {
        id: Uuid::from_u128(100),
        execution_step_id: step_id,
        source: "query_data".into(),
        source_ref: source_ref.into(),
        observed_at: timestamp,
        published_at: Some(timestamp),
        effective_at: Some(timestamp),
        snapshot: json!({
            "bars": [{
                "timestamp": timestamp,
                "open": 11.5,
                "high": 13.5,
                "low": 10.5,
                "close": 12.5,
                "volume": 100,
            }],
        }),
    }
}

fn create_command() -> CreateAnnotationCommand {
    CreateAnnotationCommand {
        actor: Actor::Llm { label: "analyst" },
        target_symbol: " FICTIONAL-ASSET ".into(),
        target_kind: " test-kind ".into(),
        timestamp: fixed_timestamp(),
        timestamp_start: None,
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
        _limit: u64,
    ) -> Result<Vec<RecentAnnotation>, AnnotationReadQueryError> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn get_annotation_returns_the_requested_annotation() {
    let target = annotation(Uuid::from_u128(40), Uuid::from_u128(41), "task");
    let query = Arc::new(FakeAnnotationReadQuery {
        annotation: target.clone(),
        listed_queries: Mutex::new(Vec::new()),
    });
    let use_cases = AnnotationReadUseCases::new(query);

    let result = use_cases
        .get_annotation(target.id)
        .await
        .expect("the annotation is readable");

    assert_eq!(result, target);
}

#[tokio::test]
async fn list_annotations_preserves_the_query() {
    let annotation = annotation(Uuid::from_u128(44), Uuid::from_u128(45), "task");
    let query = Arc::new(FakeAnnotationReadQuery {
        annotation,
        listed_queries: Mutex::new(Vec::new()),
    });
    let use_cases = AnnotationReadUseCases::new(query.clone());
    let requested_query = AnnotationListQuery {
        target_symbol: Some("FICTIONAL-ASSET".into()),
        limit: Some(5),
    };

    use_cases
        .list_annotations(requested_query.clone())
        .await
        .expect("listing succeeds");

    assert_eq!(
        query.listed_queries.lock().await.clone(),
        vec![requested_query]
    );
}

#[tokio::test]
async fn create_replaces_only_uncommented_stale_annotations_and_records_history_in_one_transaction()
{
    let (use_cases, unit_of_work, repository, change_history) = build_use_cases();
    let step_id = Uuid::from_u128(2);
    let removable_id = Uuid::from_u128(3);
    let commented_id = Uuid::from_u128(4);
    let current_attempt_id = Uuid::from_u128(5);
    let other_step_id = Uuid::from_u128(6);
    repository
        .insert_annotation(annotation(removable_id, step_id, "old-attempt"))
        .await;
    repository
        .insert_annotation(annotation(commented_id, step_id, "old-attempt"))
        .await;
    repository.set_comment(commented_id).await;
    repository
        .insert_annotation(annotation(current_attempt_id, step_id, "new-attempt"))
        .await;
    repository
        .insert_annotation(annotation(other_step_id, Uuid::from_u128(7), "old-attempt"))
        .await;
    let mut command = create_command();
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

    assert_eq!(
        (
            remaining_ids,
            unit_of_work.begun.lock().await.len(),
            unit_of_work.committed.lock().await.clone(),
            repository_transactions,
            history,
        ),
        (
            expected_remaining_ids,
            1,
            vec![transaction_id],
            vec![true, true, true, true],
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
                    json!({ "target_symbol": "FICTIONAL-ASSET" }),
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
    let (use_cases, unit_of_work, _, _) = build_use_cases();
    let mut command = create_command();
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

#[rstest]
#[case::open(PriceReferenceField::Open, Decimal::new(115, 1))]
#[case::high(PriceReferenceField::High, Decimal::new(135, 1))]
#[case::low(PriceReferenceField::Low, Decimal::new(105, 1))]
#[case::close(PriceReferenceField::Close, Decimal::new(125, 1))]
#[tokio::test]
async fn create_resolves_the_selected_price_field_from_execution_evidence(
    #[case] field: PriceReferenceField,
    #[case] expected_price: Decimal,
) {
    let step_id = Uuid::from_u128(80);
    let evidence = Arc::new(FakeStrategyTaskStepEvidenceRepository::new(vec![
        query_data_evidence(step_id, "FICTIONAL-ASSET"),
    ]));
    let (use_cases, unit_of_work, repository, _) = build_use_cases_with_evidence(evidence);
    let mut command = create_command();
    command.execution_step_id = Some(step_id);
    command.price = Some(AnnotationPriceInput::Field(field));

    let created = use_cases
        .create(command)
        .await
        .expect("price field resolves");
    let stored_price = repository
        .annotations
        .lock()
        .await
        .get(&created.id)
        .and_then(|annotation| annotation.price);

    assert_eq!(
        (
            created.price,
            stored_price,
            unit_of_work.committed.lock().await.len()
        ),
        (Some(expected_price), Some(expected_price), 1),
    );
}

#[tokio::test]
async fn create_rejects_unresolved_price_before_replacing_stale_annotations() {
    let step_id = Uuid::from_u128(81);
    let evidence = Arc::new(FakeStrategyTaskStepEvidenceRepository::default());
    let (use_cases, unit_of_work, repository, change_history) =
        build_use_cases_with_evidence(evidence);
    let stale_id = Uuid::from_u128(82);
    repository
        .insert_annotation(annotation(stale_id, step_id, "previous-task"))
        .await;
    let mut command = create_command();
    command.execution_step_id = Some(step_id);
    command.execution_task_id = Some("current-task".into());
    command.price = Some(AnnotationPriceInput::Field(PriceReferenceField::Close));

    let result = use_cases.create(command).await;
    let annotations = repository.annotations.lock().await;

    assert_eq!(
        (
            matches!(result, Err(AnnotationUseCaseError::Validation(_))),
            annotations.len(),
            annotations.contains_key(&stale_id),
            unit_of_work.committed.lock().await.len(),
            change_history.entries.lock().await.len(),
        ),
        (true, 1, true, 0, 0),
    );
}

#[tokio::test]
async fn create_rejects_price_field_without_execution_step_before_opening_transaction() {
    let (use_cases, unit_of_work, repository, _) = build_use_cases();
    let mut command = create_command();
    command.price = Some(AnnotationPriceInput::Field(PriceReferenceField::Close));

    let result = use_cases.create(command).await;

    assert_eq!(
        (
            matches!(result, Err(AnnotationUseCaseError::Validation(_))),
            unit_of_work.begun.lock().await.len(),
            repository.annotations.lock().await.len(),
        ),
        (true, 0, 0),
    );
}

#[tokio::test]
async fn create_rejects_timestamp_start_after_timestamp_before_opening_transaction() {
    let (use_cases, unit_of_work, repository, _) = build_use_cases();
    let mut command = create_command();
    command.timestamp_start = Some("1970-01-02T00:00:00Z".parse().expect("timestamp parses"));

    let result = use_cases.create(command).await;

    assert_eq!(
        (
            result.map(|_| ()).map_err(|error| match error {
                AnnotationUseCaseError::Validation(message) => message,
                _ => "unexpected error".into(),
            }),
            unit_of_work.begun.lock().await.len(),
            repository.annotations.lock().await.len(),
        ),
        (
            Err("timestamp_start must not be after timestamp".into()),
            0,
            0,
        ),
    );
}

#[rstest]
#[case::matches_query_data_range_start(Some("1970-01-01"), true, true)]
#[case::different_from_query_data_range_start(Some("1970-01-02"), true, false)]
#[case::missing_query_data_range_start(None, true, false)]
#[case::omitted_timestamp_start(Some("1970-01-01"), false, false)]
#[tokio::test]
async fn create_with_warnings_reports_when_timestamp_start_matches_the_requested_range(
    #[case] range_start: Option<&str>,
    #[case] include_timestamp_start: bool,
    #[case] should_warn: bool,
) {
    let step_id = Uuid::from_u128(84);
    let timestamp = fixed_timestamp();
    let mut evidence = query_data_evidence(step_id, "FICTIONAL-ASSET");
    if let Some(range_start) = range_start {
        evidence.snapshot["from"] = json!(range_start);
    }
    let evidence = Arc::new(FakeStrategyTaskStepEvidenceRepository::new(vec![evidence]));
    let (use_cases, unit_of_work, repository, _) = build_use_cases_with_evidence(evidence);
    let mut command = create_command();
    command.execution_step_id = Some(step_id);
    command.timestamp_start = include_timestamp_start.then_some(timestamp);

    let result = use_cases
        .create_with_warnings(command)
        .await
        .expect("annotation creation succeeds");
    let expected_warnings = if should_warn {
        vec!["timestamp_start がこの実行の query_data の取得開始日と一致しています。観測期間ではなく、アノテーション自身が語る期間の開始日か確認してください。".to_string()]
    } else {
        Vec::new()
    };

    assert_eq!(
        (
            result.annotation.timestamp_start,
            result.warnings,
            repository.annotations.lock().await.len(),
            unit_of_work.committed.lock().await.len(),
        ),
        (
            include_timestamp_start.then_some(timestamp),
            expected_warnings,
            1,
            1,
        ),
    );
}

#[tokio::test]
async fn create_resolves_price_using_the_utc_date_of_the_annotation_timestamp() {
    let step_id = Uuid::from_u128(83);
    let mut evidence = query_data_evidence(step_id, "FICTIONAL-ASSET");
    evidence.snapshot = json!({
        "bars": [
            {
                "timestamp": "2030-01-01T00:00:00Z",
                "open": 11.5,
                "high": 13.5,
                "low": 10.5,
                "close": 12.5,
                "volume": 100,
            },
            {
                "timestamp": "2030-01-02T00:00:00Z",
                "open": 21.5,
                "high": 23.5,
                "low": 20.5,
                "close": 22.5,
                "volume": 200,
            },
        ],
    });
    let evidence = Arc::new(FakeStrategyTaskStepEvidenceRepository::new(vec![evidence]));
    let (use_cases, _, _, _) = build_use_cases_with_evidence(evidence);
    let mut command = create_command();
    command.execution_step_id = Some(step_id);
    command.timestamp = "2030-01-02T00:00:00+09:00"
        .parse()
        .expect("timestamp with a non-UTC offset");
    command.price = Some(AnnotationPriceInput::Field(PriceReferenceField::Close));

    let created = use_cases.create(command).await.expect("close resolves");

    assert_eq!(created.price, Some(Decimal::new(125, 1)));
}

#[tokio::test]
async fn create_rejects_volume_as_a_price_field_before_opening_transaction() {
    let (use_cases, unit_of_work, repository, _) = build_use_cases();
    let mut command = create_command();
    command.price = Some(AnnotationPriceInput::Field(PriceReferenceField::Volume));

    let result = use_cases.create(command).await;

    assert_eq!(
        (
            matches!(result, Err(AnnotationUseCaseError::Validation(_))),
            unit_of_work.begun.lock().await.len(),
            repository.annotations.lock().await.len(),
        ),
        (true, 0, 0),
    );
}

#[tokio::test]
async fn create_accepts_an_existing_linked_note() {
    let (use_cases, unit_of_work, repository, change_history) = build_use_cases();
    let note_id = Uuid::from_u128(22);
    repository.set_note_exists(note_id).await;
    let mut command = create_command();
    command.linked_note_id = Some(note_id);

    let created = use_cases.create(command).await.expect("linked note exists");

    assert_eq!(
        (
            created.linked_note_id,
            repository.annotations.lock().await.len(),
            change_history.entries.lock().await.len(),
            unit_of_work.committed.lock().await.len(),
        ),
        (Some(note_id), 1, 1, 1),
    );
}

#[tokio::test]
async fn create_rejects_a_linked_note_that_does_not_exist() {
    let (use_cases, unit_of_work, repository, change_history) = build_use_cases();
    let note_id = Uuid::from_u128(23);
    let mut command = create_command();
    command.linked_note_id = Some(note_id);

    let result = use_cases.create(command).await;

    assert_eq!(
        (
            matches!(result, Err(AnnotationUseCaseError::LinkedNoteNotFound(id)) if id == note_id),
            unit_of_work.begun.lock().await.len(),
            unit_of_work.committed.lock().await.len(),
            repository.annotations.lock().await.len(),
            change_history.entries.lock().await.len(),
        ),
        (true, 1, 0, 0, 0),
    );
}

#[tokio::test]
async fn update_changes_an_existing_annotation() {
    let (use_cases, unit_of_work, repository, change_history) = build_use_cases();
    let id = Uuid::from_u128(30);
    repository
        .insert_annotation(annotation(id, Uuid::from_u128(33), "task"))
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
        .expect("annotation is updateable");

    assert_eq!(
        (
            updated.id,
            updated.text,
            *repository.update_calls.lock().await,
            change_history.entries.lock().await.len(),
            unit_of_work.committed.lock().await.len(),
        ),
        (id, "updated text".into(), 1, 1, 1),
    );
}

#[tokio::test]
async fn update_rejects_timestamp_before_existing_timestamp_start() {
    let (use_cases, unit_of_work, repository, change_history) = build_use_cases();
    let id = Uuid::from_u128(31);
    let mut existing = annotation(id, Uuid::from_u128(33), "task");
    existing.timestamp = "1970-01-03T00:00:00Z".parse().expect("timestamp parses");
    existing.timestamp_start = Some("1970-01-02T00:00:00Z".parse().expect("timestamp parses"));
    repository.insert_annotation(existing.clone()).await;

    let result = use_cases
        .update(UpdateAnnotationCommand {
            actor: Actor::Llm { label: "analyst" },
            id,
            target_symbol: None,
            target_kind: None,
            timestamp: Some(fixed_timestamp()),
            price: None,
            text: None,
            linked_note_id: None,
        })
        .await;
    let stored = repository.annotations.lock().await.get(&id).cloned();

    assert_eq!(
        (
            result.map(|_| ()).map_err(|error| match error {
                AnnotationUseCaseError::Validation(message) => message,
                _ => "unexpected error".into(),
            }),
            stored,
            *repository.update_calls.lock().await,
            change_history.entries.lock().await.len(),
            unit_of_work.committed.lock().await.len(),
        ),
        (
            Err("timestamp_start must not be after timestamp".into()),
            Some(existing),
            0,
            0,
            0,
        ),
    );
}

#[tokio::test]
async fn change_status_does_not_update_or_record_history_when_status_is_unchanged() {
    let (use_cases, unit_of_work, repository, change_history) = build_use_cases();
    let id = Uuid::from_u128(30);
    let existing = Annotation {
        id,
        target_symbol: "FICTIONAL-ASSET".into(),
        target_kind: "test-kind".into(),
        timestamp: fixed_timestamp(),
        timestamp_start: None,
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
