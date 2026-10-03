use core_application::change_history::Actor;
use core_application::note::NoteWriteCommand;
use core_application::strategy_scope::StrategyScope;
use gateway_postgres::DatabaseHandle;
use gateway_postgres::entities::note_version;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

struct TestNoteOptions<'a> {
    strategy_id: Option<Uuid>,
    title: &'a str,
    body_md: &'a str,
    execution_id: Option<String>,
    created_by_kind: &'a str,
    status: &'a str,
    actor: Actor,
}

pub async fn insert_test_note(
    db: &DatabaseHandle,
    strategy_id: Uuid,
    title: &str,
    body_md: &str,
) -> Uuid {
    insert_test_note_in_scope(db, Some(strategy_id), title, body_md).await
}

pub async fn insert_test_note_in_scope(
    db: &DatabaseHandle,
    strategy_id: Option<Uuid>,
    title: &str,
    body_md: &str,
) -> Uuid {
    insert_test_note_with_options(
        db,
        TestNoteOptions {
            strategy_id,
            title,
            body_md,
            execution_id: None,
            created_by_kind: "human",
            status: "unread",
            actor: Actor::Human,
        },
    )
    .await
}

pub async fn insert_test_note_as(
    db: &DatabaseHandle,
    strategy_id: Option<Uuid>,
    title: &str,
    body_md: &str,
    created_by_kind: &str,
    actor: Actor,
) -> Uuid {
    insert_test_note_with_options(
        db,
        TestNoteOptions {
            strategy_id,
            title,
            body_md,
            execution_id: None,
            created_by_kind,
            status: "unread",
            actor,
        },
    )
    .await
}

pub async fn insert_test_note_with_status(
    db: &DatabaseHandle,
    strategy_id: Uuid,
    title: &str,
    body_md: &str,
    status: &str,
) -> Uuid {
    insert_test_note_with_options(
        db,
        TestNoteOptions {
            strategy_id: Some(strategy_id),
            title,
            body_md,
            execution_id: None,
            created_by_kind: "human",
            status,
            actor: Actor::Human,
        },
    )
    .await
}

pub async fn insert_test_note_with_execution_id(
    db: &DatabaseHandle,
    strategy_id: Uuid,
    title: &str,
    body_md: &str,
    execution_id: &str,
) -> Uuid {
    insert_test_note_with_options(
        db,
        TestNoteOptions {
            strategy_id: Some(strategy_id),
            title,
            body_md,
            execution_id: Some(execution_id.to_string()),
            created_by_kind: "llm",
            status: "unread",
            actor: Actor::Human,
        },
    )
    .await
}

pub async fn set_test_note_version_execution_id(
    db: &DatabaseHandle,
    version_id: Uuid,
    execution_step_id: Uuid,
) {
    note_version::Entity::update_many()
        .col_expr(
            note_version::Column::ExecutionId,
            Expr::value(Some(execution_step_id.to_string())),
        )
        .filter(note_version::Column::Id.eq(version_id))
        .exec(db)
        .await
        .expect("set test note version execution ID");
}

async fn insert_test_note_with_options(db: &DatabaseHandle, options: TestNoteOptions<'_>) -> Uuid {
    let scope = options.execution_id.as_ref().map(|_| {
        StrategyScope::from(
            options
                .strategy_id
                .expect("execution notes have a strategy"),
        )
    });
    let result = crate::services::use_cases::build_use_cases(db.clone())
        .notes()
        .write(NoteWriteCommand {
            scope,
            strategy_id: options.strategy_id,
            execution_id: options.execution_id,
            note_id: None,
            title: Some(options.title.to_string()),
            body_md: Some(options.body_md.to_string()),
            frontmatter_json: Some(serde_json::json!({})),
            graphs_json: Some(serde_json::json!([])),
            kind: None,
            status: None,
            trigger: None,
            trigger_label: None,
            created_by_kind: options.created_by_kind.to_string(),
            change_reason: None,
            actor: options.actor,
            change_diff: None,
        })
        .await
        .expect("create test note");

    if result.snapshot.version.status != options.status {
        note_version::ActiveModel {
            id: Set(result.snapshot.version.id),
            status: Set(options.status.to_string()),
            ..Default::default()
        }
        .update(db)
        .await
        .expect("set test note version status");
    }

    result.note_id
}

pub async fn find_current_note_version<C: ConnectionTrait>(
    db: &C,
    note_id: Uuid,
) -> Result<Option<note_version::Model>, sea_orm::DbErr> {
    note_version::Entity::find()
        .filter(note_version::Column::NoteId.eq(note_id))
        .filter(note_version::Column::IsCurrent.eq(true))
        .one(db)
        .await
}
