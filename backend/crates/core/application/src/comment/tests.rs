use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rstest::{fixture, rstest};
use serde_json::json;
use uuid::Uuid;

use crate::change_history::{
    Actor, ChangeHistoryError, ChangeHistoryPort, ChangeHistoryRecord, Op, TargetKind,
};
use crate::strategy_scope::{StrategyScope, StrategyScopeSource, StrategyScopeSourceError};
use crate::unit_of_work::{UnitOfWork, UnitOfWorkError, UnitOfWorkTransaction};

use super::error::CommentUseCaseError;
use super::fake::{FakeCommentRepository, FakeCommentTransaction};
use super::types::{
    Comment, CommentTargetKind, CreateCommentCommand, DeleteCommentCommand,
    NoteVersionAnchorBodies, ReplyCommentCommand, ResolveCommentCommand,
};
use super::use_cases::CommentUseCases;

const TARGET_ID: Uuid = Uuid::from_u128(10);
const OTHER_TARGET_ID: Uuid = Uuid::from_u128(11);
const PARENT_ID: Uuid = Uuid::from_u128(12);
const GRANDPARENT_ID: Uuid = Uuid::from_u128(13);
const STRATEGY_ID: Uuid = Uuid::from_u128(14);
const OTHER_STRATEGY_ID: Uuid = Uuid::from_u128(15);
const NORMALIZED_ID: Uuid = Uuid::from_u128(16);

#[derive(Debug, Clone, PartialEq)]
struct RecordedChange {
    transaction_id: Uuid,
    record: ChangeHistoryRecord,
}

#[derive(Default)]
struct FakeChangeHistory {
    records: Mutex<Vec<RecordedChange>>,
}

impl FakeChangeHistory {
    fn records(&self) -> Vec<RecordedChange> {
        lock(&self.records).clone()
    }
}

#[async_trait]
impl ChangeHistoryPort for FakeChangeHistory {
    async fn record(
        &self,
        transaction: &UnitOfWorkTransaction,
        record: ChangeHistoryRecord,
    ) -> Result<(), ChangeHistoryError> {
        let transaction_id = transaction
            .downcast_ref::<FakeCommentTransaction>()
            .map(|transaction| transaction.id)
            .ok_or(ChangeHistoryError::InvalidTransaction)?;
        lock(&self.records).push(RecordedChange {
            transaction_id,
            record,
        });
        Ok(())
    }
}

#[derive(Default)]
struct FakeUnitOfWork {
    started: Mutex<usize>,
    committed: Mutex<Vec<Uuid>>,
}

impl FakeUnitOfWork {
    fn started(&self) -> usize {
        *lock(&self.started)
    }

    fn committed(&self) -> Vec<Uuid> {
        lock(&self.committed).clone()
    }
}

#[async_trait]
impl UnitOfWork for FakeUnitOfWork {
    async fn begin(&self) -> Result<UnitOfWorkTransaction, UnitOfWorkError> {
        *lock(&self.started) += 1;
        Ok(UnitOfWorkTransaction::new(FakeCommentTransaction {
            id: Uuid::new_v4(),
        }))
    }

    async fn commit(&self, transaction: UnitOfWorkTransaction) -> Result<(), UnitOfWorkError> {
        let transaction = transaction
            .downcast::<FakeCommentTransaction>()
            .map_err(|_| UnitOfWorkError::InvalidTransaction)?;
        lock(&self.committed).push(transaction.id);
        Ok(())
    }
}

struct Fixture {
    use_cases: CommentUseCases,
    repository: Arc<FakeCommentRepository>,
    unit_of_work: Arc<FakeUnitOfWork>,
    history: Arc<FakeChangeHistory>,
}

#[rstest]
#[case::empty_body("note_version", " \n ", "body must not be empty")]
#[case::invalid_target("stock", "comment", "invalid target_kind: stock")]
#[tokio::test]
async fn create_rejects_invalid_input_before_opening_a_transaction(
    fixture: Fixture,
    #[case] target_kind: &str,
    #[case] body: &str,
    #[case] expected_error: &str,
) {
    let result = fixture
        .use_cases
        .create(create_command(target_kind, TARGET_ID, body))
        .await;

    assert_eq!(
        (
            validation_error(result),
            fixture.unit_of_work.started(),
            fixture.history.records(),
        ),
        (Some(expected_error.to_string()), 0, Vec::new()),
    );
}

#[rstest]
#[case::different_target(
    "annotation",
    OTHER_TARGET_ID,
    None,
    "parent comment belongs to a different target"
)]
#[case::reply_to_reply(
    "note_version",
    TARGET_ID,
    Some(GRANDPARENT_ID),
    "cannot reply to a reply; parent_id must reference a top-level comment"
)]
#[tokio::test]
async fn create_rejects_a_reply_that_does_not_target_a_top_level_parent(
    fixture: Fixture,
    #[case] parent_target_kind: &str,
    #[case] parent_target_id: Uuid,
    #[case] parent_id: Option<Uuid>,
    #[case] expected_error: &str,
) {
    let parent = comment(PARENT_ID, parent_target_kind, parent_target_id, parent_id);
    fixture.repository.insert_existing(parent);

    let result = fixture
        .use_cases
        .create(CreateCommentCommand {
            parent_id: Some(PARENT_ID),
            ..create_command("note_version", TARGET_ID, "reply body")
        })
        .await;

    assert_eq!(
        (
            validation_error(result),
            fixture.repository.comments().len(),
            fixture.history.records(),
            fixture.unit_of_work.committed(),
        ),
        (Some(expected_error.to_string()), 1, Vec::new(), Vec::new(),),
    );
}

#[rstest]
#[tokio::test]
async fn create_reports_a_missing_parent_as_validation(fixture: Fixture) {
    let result = fixture
        .use_cases
        .create(CreateCommentCommand {
            parent_id: Some(PARENT_ID),
            ..create_command("note_version", TARGET_ID, "reply body")
        })
        .await;

    assert_eq!(
        validation_error(result),
        Some(format!("parent comment {PARENT_ID} not found")),
    );
}

#[rstest]
#[tokio::test]
async fn reply_reports_a_missing_parent_as_not_found(fixture: Fixture) {
    let result = fixture
        .use_cases
        .reply(ReplyCommentCommand {
            parent_id: PARENT_ID,
            body: "reply body".into(),
            author_kind: "llm".into(),
            author_label: "analyst".into(),
            scope: None,
            actor: Actor::Llm { label: "analyst" },
        })
        .await;

    assert_eq!(
        not_found_error(result),
        Some(format!("parent comment {PARENT_ID} not found")),
    );
}

#[rstest]
#[tokio::test]
async fn reply_rejects_a_reply_parent(fixture: Fixture) {
    fixture.repository.insert_existing(comment(
        PARENT_ID,
        "note_version",
        TARGET_ID,
        Some(GRANDPARENT_ID),
    ));

    let result = fixture
        .use_cases
        .reply(ReplyCommentCommand {
            parent_id: PARENT_ID,
            body: "reply body".into(),
            author_kind: "llm".into(),
            author_label: "analyst".into(),
            scope: None,
            actor: Actor::Llm { label: "analyst" },
        })
        .await;

    assert_eq!(
        (
            validation_error(result),
            fixture.repository.comments().len(),
            fixture.history.records(),
        ),
        (
            Some("cannot reply to a reply; parent_id must reference a top-level comment".into(),),
            1,
            Vec::new(),
        ),
    );
}

#[rstest]
#[case::annotation_anchor(
    "annotation",
    Some("new"),
    Some(1),
    Some(1),
    "line anchors are only supported for note_version comments"
)]
#[case::incomplete_range(
    "note_version",
    Some("new"),
    Some(1),
    None,
    "anchor_side, start_line, and end_line must be provided together"
)]
#[case::invalid_side(
    "note_version",
    Some("middle"),
    Some(1),
    Some(1),
    "anchor_side must be either old or new"
)]
#[tokio::test]
async fn create_rejects_invalid_line_anchor_shapes(
    fixture: Fixture,
    #[case] target_kind: &str,
    #[case] anchor_side: Option<&str>,
    #[case] start_line: Option<i32>,
    #[case] end_line: Option<i32>,
    #[case] expected_error: &str,
) {
    fixture.repository.set_note_version_anchor_bodies(
        TARGET_ID,
        NoteVersionAnchorBodies {
            version_no: 2,
            current_body: "line".into(),
            previous_body: Some("line".into()),
        },
    );
    let mut command = create_command(target_kind, TARGET_ID, "comment");
    command.anchor_side = anchor_side.map(ToOwned::to_owned);
    command.start_line = start_line;
    command.end_line = end_line;

    let result = fixture.use_cases.create(command).await;

    assert_eq!(
        (validation_error(result), fixture.history.records()),
        (Some(expected_error.into()), Vec::new()),
    );
}

#[rstest]
#[case::old_side("old", "current body", "previous body\nsecond line")]
#[case::new_side("new", "current body\nsecond line", "previous body")]
#[tokio::test]
async fn create_persists_a_validated_note_version_anchor(
    fixture: Fixture,
    #[case] anchor_side: &str,
    #[case] current_body: &str,
    #[case] previous_body: &str,
) {
    fixture.repository.set_note_version_anchor_bodies(
        TARGET_ID,
        NoteVersionAnchorBodies {
            version_no: 2,
            current_body: current_body.into(),
            previous_body: Some(previous_body.into()),
        },
    );
    let mut command = create_command("note_version", TARGET_ID, "comment");
    command.anchor_text = Some(" selected text ".into());
    command.anchor_side = Some(anchor_side.to_string());
    command.start_line = Some(2);
    command.end_line = Some(2);

    let created = fixture
        .use_cases
        .create(command)
        .await
        .expect("comment is created");
    let normalized = normalize_comment(created);

    assert_eq!(
        (
            normalized,
            fixture.history.records().len(),
            fixture.unit_of_work.committed().len(),
            fixture
                .repository
                .transaction_ids()
                .iter()
                .all(|id| { fixture.unit_of_work.committed().contains(id) }),
        ),
        (
            Comment {
                id: NORMALIZED_ID,
                target_kind: "note_version".into(),
                target_id: TARGET_ID,
                parent_id: None,
                body: "comment".into(),
                author_kind: "human".into(),
                author_label: "reviewer".into(),
                resolved: false,
                created_at: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset(),
                anchor_text: Some(" selected text ".into()),
                anchor_side: Some(anchor_side.into()),
                start_line: Some(2),
                end_line: Some(2),
            },
            1,
            1,
            true,
        ),
    );
}

#[rstest]
#[case::new_side("new", 2, "single line", None)]
#[case::old_side("old", 2, "single line", Some("single line"))]
#[tokio::test]
async fn create_rejects_a_line_range_past_the_selected_body(
    fixture: Fixture,
    #[case] anchor_side: &str,
    #[case] version_no: i32,
    #[case] current_body: &str,
    #[case] previous_body: Option<&str>,
) {
    fixture.repository.set_note_version_anchor_bodies(
        TARGET_ID,
        NoteVersionAnchorBodies {
            version_no,
            current_body: current_body.into(),
            previous_body: previous_body.map(ToOwned::to_owned),
        },
    );
    let mut command = create_command("note_version", TARGET_ID, "comment");
    command.anchor_side = Some(anchor_side.into());
    command.start_line = Some(1);
    command.end_line = Some(2);

    let result = fixture.use_cases.create(command).await;

    assert_eq!(
        (
            validation_error(result),
            fixture.repository.comments(),
            fixture.history.records(),
        ),
        (
            Some("line anchor ends at 2, but the selected version has 1 lines".into(),),
            Vec::new(),
            Vec::new(),
        ),
    );
}

#[rstest]
#[tokio::test]
async fn reply_inherits_parent_target_and_records_the_supplied_actor(fixture: Fixture) {
    fixture
        .repository
        .insert_existing(comment(PARENT_ID, "note_version", TARGET_ID, None));

    let created = fixture
        .use_cases
        .reply(ReplyCommentCommand {
            parent_id: PARENT_ID,
            body: "reply body".into(),
            author_kind: "llm".into(),
            author_label: "analyst".into(),
            scope: None,
            actor: Actor::Llm { label: "analyst" },
        })
        .await
        .expect("reply is created");
    let history = fixture
        .history
        .records()
        .into_iter()
        .map(normalize_record)
        .collect::<Vec<_>>();
    let transaction_id = fixture.unit_of_work.committed()[0];

    assert_eq!(
        (
            normalize_comment(created),
            history,
            fixture
                .repository
                .transaction_ids()
                .iter()
                .all(|id| *id == transaction_id),
            fixture.unit_of_work.committed(),
        ),
        (
            Comment {
                id: NORMALIZED_ID,
                target_kind: "note_version".into(),
                target_id: TARGET_ID,
                parent_id: Some(PARENT_ID),
                body: "reply body".into(),
                author_kind: "llm".into(),
                author_label: "analyst".into(),
                resolved: false,
                created_at: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset(),
                anchor_text: None,
                anchor_side: None,
                start_line: None,
                end_line: None,
            },
            vec![RecordedChange {
                transaction_id,
                record: ChangeHistoryRecord {
                    actor: Actor::Llm { label: "analyst" },
                    target_kind: TargetKind::Comment,
                    target_id: NORMALIZED_ID,
                    op: Op::Create,
                    diff: json!({
                        "target_kind": "note_version",
                        "target_id": TARGET_ID,
                        "parent_id": PARENT_ID,
                    }),
                    summary: None,
                },
            }],
            true,
            vec![transaction_id],
        ),
    );
}

#[rstest]
#[tokio::test]
async fn resolve_skips_update_and_history_when_status_is_unchanged(fixture: Fixture) {
    let existing = comment(PARENT_ID, "annotation", TARGET_ID, None);
    fixture.repository.insert_existing(existing.clone());

    let resolved = fixture
        .use_cases
        .resolve(ResolveCommentCommand {
            id: PARENT_ID,
            resolved: false,
            scope: None,
            actor: Actor::Human,
        })
        .await
        .expect("comment is returned");

    assert_eq!(
        (
            resolved,
            fixture.repository.update_count(),
            fixture.history.records(),
            fixture.unit_of_work.committed().len(),
        ),
        (existing, 0, Vec::new(), 1),
    );
}

#[rstest]
#[tokio::test]
async fn resolve_records_a_status_change_in_the_write_transaction(fixture: Fixture) {
    fixture
        .repository
        .insert_existing(comment(PARENT_ID, "annotation", TARGET_ID, None));

    let updated = fixture
        .use_cases
        .resolve(ResolveCommentCommand {
            id: PARENT_ID,
            resolved: true,
            scope: None,
            actor: Actor::Llm { label: "analyst" },
        })
        .await
        .expect("comment is resolved");
    let transaction_id = fixture.unit_of_work.committed()[0];

    assert_eq!(
        (
            updated.resolved,
            fixture.repository.update_count(),
            fixture.history.records(),
            fixture
                .repository
                .transaction_ids()
                .iter()
                .all(|id| *id == transaction_id),
        ),
        (
            true,
            1,
            vec![RecordedChange {
                transaction_id,
                record: ChangeHistoryRecord {
                    actor: Actor::Llm { label: "analyst" },
                    target_kind: TargetKind::Comment,
                    target_id: PARENT_ID,
                    op: Op::StatusChange,
                    diff: json!({ "from": false, "to": true }),
                    summary: None,
                },
            }],
            true,
        ),
    );
}

#[rstest]
#[tokio::test]
async fn delete_records_history_with_the_supplied_actor(fixture: Fixture) {
    fixture
        .repository
        .insert_existing(comment(PARENT_ID, "annotation", TARGET_ID, None));

    fixture
        .use_cases
        .delete(DeleteCommentCommand {
            id: PARENT_ID,
            scope: None,
            actor: Actor::Llm { label: "analyst" },
        })
        .await
        .expect("comment is deleted");
    let transaction_id = fixture.unit_of_work.committed()[0];

    assert_eq!(
        (
            fixture.repository.comments(),
            fixture.history.records(),
            fixture
                .repository
                .transaction_ids()
                .iter()
                .all(|id| *id == transaction_id),
        ),
        (
            Vec::new(),
            vec![RecordedChange {
                transaction_id,
                record: ChangeHistoryRecord {
                    actor: Actor::Llm { label: "analyst" },
                    target_kind: TargetKind::Comment,
                    target_id: PARENT_ID,
                    op: Op::Delete,
                    diff: json!({}),
                    summary: None,
                },
            }],
            true,
        ),
    );
}

#[rstest]
#[tokio::test]
async fn scoped_creation_rejects_a_target_owned_by_another_strategy(fixture: Fixture) {
    fixture.repository.set_target_strategy_id(
        CommentTargetKind::NoteVersion,
        TARGET_ID,
        OTHER_STRATEGY_ID,
    );
    let scope = verified_scope(STRATEGY_ID).await;
    let mut command = create_command("note_version", TARGET_ID, "comment");
    command.scope = Some(scope);

    let result = fixture.use_cases.create(command).await;

    assert_eq!(
        (
            forbidden_error(result),
            fixture.repository.comments(),
            fixture.history.records(),
            fixture.unit_of_work.committed(),
        ),
        (
            Some("comment target belongs to a different strategy".into()),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
    );
}

#[fixture]
fn fixture() -> Fixture {
    let repository = Arc::new(FakeCommentRepository::new());
    let unit_of_work = Arc::new(FakeUnitOfWork::default());
    let history = Arc::new(FakeChangeHistory::default());
    let use_cases = CommentUseCases::new(unit_of_work.clone(), repository.clone(), history.clone());
    Fixture {
        use_cases,
        repository,
        unit_of_work,
        history,
    }
}

struct FixedScopeSource {
    ids: HashSet<Uuid>,
}

#[async_trait]
impl StrategyScopeSource for FixedScopeSource {
    async fn existing_ids(&self, _ids: &[Uuid]) -> Result<HashSet<Uuid>, StrategyScopeSourceError> {
        Ok(self.ids.clone())
    }
}

async fn verified_scope(id: Uuid) -> StrategyScope {
    StrategyScope::verify(
        id,
        &FixedScopeSource {
            ids: HashSet::from([id]),
        },
    )
    .await
    .expect("strategy exists")
}

fn create_command(target_kind: &str, target_id: Uuid, body: &str) -> CreateCommentCommand {
    CreateCommentCommand {
        target_kind: target_kind.into(),
        target_id,
        parent_id: None,
        body: body.into(),
        author_kind: "human".into(),
        author_label: "reviewer".into(),
        anchor_text: None,
        anchor_side: None,
        start_line: None,
        end_line: None,
        scope: None,
        actor: Actor::Human,
    }
}

fn comment(id: Uuid, target_kind: &str, target_id: Uuid, parent_id: Option<Uuid>) -> Comment {
    Comment {
        id,
        target_kind: target_kind.into(),
        target_id,
        parent_id,
        body: "existing comment".into(),
        author_kind: "human".into(),
        author_label: "reviewer".into(),
        resolved: false,
        created_at: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset(),
        anchor_text: None,
        anchor_side: None,
        start_line: None,
        end_line: None,
    }
}

fn normalize_comment(mut comment: Comment) -> Comment {
    comment.id = NORMALIZED_ID;
    comment
}

fn normalize_record(mut entry: RecordedChange) -> RecordedChange {
    entry.record.target_id = NORMALIZED_ID;
    entry
}

fn validation_error<T>(result: Result<T, CommentUseCaseError>) -> Option<String> {
    match result {
        Err(CommentUseCaseError::Validation(message)) => Some(message),
        _ => None,
    }
}

fn not_found_error<T>(result: Result<T, CommentUseCaseError>) -> Option<String> {
    match result {
        Err(CommentUseCaseError::NotFound(message)) => Some(message),
        _ => None,
    }
}

fn forbidden_error<T>(result: Result<T, CommentUseCaseError>) -> Option<String> {
    match result {
        Err(CommentUseCaseError::Forbidden(message)) => Some(message),
        _ => None,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
