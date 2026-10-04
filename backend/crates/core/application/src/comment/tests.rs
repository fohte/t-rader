#![cfg(feature = "test-support")]

use std::sync::Arc;

use rstest::{fixture, rstest};
use serde_json::json;
use uuid::Uuid;

use crate::change_history::{Actor, ChangeHistoryRecord, FakeChangeHistory, Op, TargetKind};
use crate::unit_of_work::FakeUnitOfWork;

use super::error::CommentUseCaseError;
use super::fake::FakeCommentRepository;
use super::types::{
    Comment, CommentTargetKind, CreateCommentCommand, DeleteCommentCommand,
    NoteVersionAnchorBodies, ReplyCommentCommand, ResolveCommentCommand,
};
use super::use_cases::CommentUseCases;

const TARGET_ID: Uuid = Uuid::from_u128(10);
const OTHER_TARGET_ID: Uuid = Uuid::from_u128(11);
const PARENT_ID: Uuid = Uuid::from_u128(12);
const GRANDPARENT_ID: Uuid = Uuid::from_u128(13);
const NORMALIZED_ID: Uuid = Uuid::from_u128(16);

type RecordedChange = crate::change_history::FakeChangeHistoryEntry;

struct Fixture {
    use_cases: CommentUseCases,
    repository: Arc<FakeCommentRepository>,
    unit_of_work: Arc<FakeUnitOfWork>,
    history: Arc<FakeChangeHistory>,
}

impl Fixture {
    async fn started(&self) -> usize {
        self.unit_of_work.begun.lock().await.len()
    }

    async fn committed(&self) -> Vec<Uuid> {
        self.unit_of_work.committed.lock().await.clone()
    }

    async fn history_records(&self) -> Vec<RecordedChange> {
        self.history.entries.lock().await.clone()
    }
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
            fixture.started().await,
            fixture.history_records().await,
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
            fixture.history_records().await,
            fixture.committed().await,
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
            require_target_exists: false,
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
            require_target_exists: false,
            actor: Actor::Llm { label: "analyst" },
        })
        .await;

    assert_eq!(
        (
            validation_error(result),
            fixture.repository.comments().len(),
            fixture.history_records().await,
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
    2,
    "line anchors are only supported for note_version comments"
)]
#[case::incomplete_range(
    "note_version",
    Some("new"),
    Some(1),
    None,
    2,
    "anchor_side, start_line, and end_line must be provided together"
)]
#[case::invalid_side(
    "note_version",
    Some("middle"),
    Some(1),
    Some(1),
    2,
    "anchor_side must be either old or new"
)]
#[case::first_version_old_side(
    "note_version",
    Some("old"),
    Some(1),
    Some(1),
    1,
    "the first version has no old-side lines"
)]
#[tokio::test]
async fn create_rejects_invalid_line_anchor_shapes(
    fixture: Fixture,
    #[case] target_kind: &str,
    #[case] anchor_side: Option<&str>,
    #[case] start_line: Option<i32>,
    #[case] end_line: Option<i32>,
    #[case] version_no: i32,
    #[case] expected_error: &str,
) {
    if target_kind == "note_version" {
        fixture.repository.set_note_version_anchor_bodies(
            TARGET_ID,
            NoteVersionAnchorBodies {
                version_no,
                current_body: "line".into(),
                previous_body: Some("line".into()),
            },
        );
    } else {
        fixture
            .repository
            .set_target_exists(CommentTargetKind::Annotation, TARGET_ID);
    }
    let mut command = create_command(target_kind, TARGET_ID, "comment");
    command.anchor_side = anchor_side.map(ToOwned::to_owned);
    command.start_line = start_line;
    command.end_line = end_line;

    let result = fixture.use_cases.create(command).await;

    assert_eq!(
        (validation_error(result), fixture.history_records().await),
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
    let history_count = fixture.history_records().await.len();
    let committed = fixture.committed().await;

    assert_eq!(
        (
            normalized,
            history_count,
            committed.len(),
            fixture
                .repository
                .transaction_ids()
                .iter()
                .all(|id| committed.contains(id)),
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
            fixture.history_records().await,
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
            require_target_exists: false,
            actor: Actor::Llm { label: "analyst" },
        })
        .await
        .expect("reply is created");
    let history = fixture
        .history_records()
        .await
        .into_iter()
        .map(normalize_record)
        .collect::<Vec<_>>();
    let committed = fixture.committed().await;
    let transaction_id = committed[0];

    assert_eq!(
        (
            normalize_comment(created),
            history,
            fixture
                .repository
                .transaction_ids()
                .iter()
                .all(|id| *id == transaction_id),
            committed,
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
            require_target_exists: false,
            actor: Actor::Human,
        })
        .await
        .expect("comment is returned");

    assert_eq!(
        (
            resolved,
            fixture.repository.update_count(),
            fixture.history_records().await,
            fixture.committed().await.len(),
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
            require_target_exists: false,
            actor: Actor::Llm { label: "analyst" },
        })
        .await
        .expect("comment is resolved");
    let transaction_id = fixture.committed().await[0];

    assert_eq!(
        (
            updated.resolved,
            fixture.repository.update_count(),
            fixture.history_records().await,
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
            actor: Actor::Llm { label: "analyst" },
        })
        .await
        .expect("comment is deleted");
    let transaction_id = fixture.committed().await[0];

    assert_eq!(
        (
            fixture.repository.comments(),
            fixture.history_records().await,
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
#[case::existing_target(true, None)]
#[case::missing_target(false, Some("comment target not found"))]
#[tokio::test]
async fn create_accepts_existing_targets_and_rejects_missing_targets(
    fixture: Fixture,
    #[case] target_exists: bool,
    #[case] expected_error: Option<&'static str>,
) {
    if target_exists {
        fixture.repository.set_note_version_anchor_bodies(
            TARGET_ID,
            NoteVersionAnchorBodies {
                version_no: 1,
                current_body: "commented version".into(),
                previous_body: None,
            },
        );
    }
    let command = create_command("note_version", TARGET_ID, "comment");

    let result = fixture.use_cases.create(command).await;

    assert_eq!(
        (
            not_found_error(result),
            fixture.repository.comments().len(),
            fixture.history_records().await.len(),
            fixture.committed().await.len(),
        ),
        (
            expected_error.map(str::to_string),
            usize::from(expected_error.is_none()),
            usize::from(expected_error.is_none()),
            usize::from(expected_error.is_none()),
        ),
    );
}

#[fixture]
fn fixture() -> Fixture {
    let repository = Arc::new(FakeCommentRepository::new());
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let history = Arc::new(FakeChangeHistory::new());
    let use_cases = CommentUseCases::new(unit_of_work.clone(), repository.clone(), history.clone());
    Fixture {
        use_cases,
        repository,
        unit_of_work,
        history,
    }
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
