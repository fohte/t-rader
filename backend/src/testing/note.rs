use core_application::change_history::Actor;
use core_application::note::NoteWriteCommand;
use core_application::strategy_scope::StrategyScope;
use gateway_postgres::DatabaseHandle;
use gateway_postgres::entities::note_version;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

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
    insert_test_note_with_options(db, strategy_id, title, body_md, None, "human", "unread").await
}

pub async fn insert_test_note_with_status(
    db: &DatabaseHandle,
    strategy_id: Uuid,
    title: &str,
    body_md: &str,
    status: &str,
) -> Uuid {
    insert_test_note_with_options(db, Some(strategy_id), title, body_md, None, "human", status)
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
        Some(strategy_id),
        title,
        body_md,
        Some(execution_id.to_string()),
        "llm",
        "unread",
    )
    .await
}

async fn insert_test_note_with_options(
    db: &DatabaseHandle,
    strategy_id: Option<Uuid>,
    title: &str,
    body_md: &str,
    execution_id: Option<String>,
    created_by_kind: &str,
    status: &str,
) -> Uuid {
    let scope = execution_id
        .as_ref()
        .map(|_| StrategyScope::from(strategy_id.expect("execution notes have a strategy")));
    let result = crate::services::use_cases::build_use_cases(db.clone())
        .notes()
        .write(NoteWriteCommand {
            scope,
            strategy_id,
            execution_id,
            note_id: None,
            title: Some(title.to_string()),
            body_md: Some(body_md.to_string()),
            frontmatter_json: Some(serde_json::json!({})),
            graphs_json: Some(serde_json::json!([])),
            kind: None,
            status: None,
            trigger: None,
            trigger_label: None,
            created_by_kind: created_by_kind.to_string(),
            change_reason: None,
            actor: Actor::Human,
            change_diff: None,
        })
        .await
        .expect("create test note");

    if result.snapshot.version.status != status {
        note_version::ActiveModel {
            id: Set(result.snapshot.version.id),
            status: Set(status.to_string()),
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
