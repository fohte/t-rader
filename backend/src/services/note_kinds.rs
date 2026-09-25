//! ノート種別の CRUD を REST と管理 MCP で共有する。

use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    IntoActiveModel, QueryFilter, QueryOrder, QuerySelect, SqlErr, TransactionTrait,
};
use serde_json::json;
use uuid::Uuid;

use crate::entities::{note, note_kind, note_version};
use crate::error::AppError;
use crate::services::change_history::{self, Actor, Op, TargetKind};
use crate::services::note_versions;

#[derive(Debug, Clone)]
pub struct CreateNoteKind {
    pub key: String,
    pub display_name: String,
    pub requires_approval: bool,
    pub description: Option<String>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateNoteKind {
    pub display_name: Option<String>,
    pub requires_approval: Option<bool>,
    pub description: Option<Option<String>>,
    pub sort_order: Option<i32>,
}

fn validate_key(key: &str) -> Result<String, AppError> {
    if key.trim().is_empty() {
        return Err(AppError::Validation("key must not be empty".into()));
    }
    Ok(key.to_string())
}

fn validate_display_name(display_name: &str) -> Result<String, AppError> {
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err(AppError::Validation(
            "display_name must not be empty".into(),
        ));
    }
    Ok(display_name.to_string())
}

async fn find_by_key<C>(db: &C, key: &str) -> Result<note_kind::Model, AppError>
where
    C: ConnectionTrait,
{
    note_kind::Entity::find_by_id(key.to_string())
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("note kind {key} not found")))
}

/// change_history.target_id が UUID 型のため、文字列主キーから安定した UUID を導出する。
fn history_target_id(key: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, format!("note_kind:{key}").as_bytes())
}

fn map_insert_error(error: DbErr, key: &str) -> AppError {
    if matches!(error.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
        return AppError::Conflict(format!("note kind {key} already exists"));
    }
    AppError::Database(error)
}

pub async fn list(db: &DatabaseConnection) -> Result<Vec<note_kind::Model>, AppError> {
    Ok(note_kind::Entity::find()
        .order_by_asc(note_kind::Column::SortOrder)
        .order_by_asc(note_kind::Column::Key)
        .all(db)
        .await?)
}

pub async fn ensure_reference<C: ConnectionTrait>(db: &C, key: &str) -> Result<(), AppError> {
    if key.trim().is_empty() {
        return Err(AppError::Validation("kind must not be empty".into()));
    }
    if note_kind::Entity::find_by_id(key.to_string())
        .one(db)
        .await?
        .is_none()
    {
        return Err(AppError::Validation(format!("unknown note kind: {key}")));
    }
    Ok(())
}

pub async fn create(
    db: &DatabaseConnection,
    actor: Actor,
    input: CreateNoteKind,
) -> Result<note_kind::Model, AppError> {
    let key = validate_key(&input.key)?;
    let display_name = validate_display_name(&input.display_name)?;
    let txn = db.begin().await?;
    let created = note_kind::Entity::insert(note_kind::ActiveModel {
        key: Set(key.clone()),
        display_name: Set(display_name),
        requires_approval: Set(input.requires_approval),
        description: Set(input.description),
        sort_order: Set(input.sort_order.unwrap_or_default()),
    })
    .exec_with_returning(&txn)
    .await
    .map_err(|error| map_insert_error(error, &key))?;

    change_history::record_as(
        &txn,
        actor,
        TargetKind::NoteKind,
        history_target_id(&key),
        Op::Create,
        json!({
            "key": created.key,
            "display_name": created.display_name,
            "requires_approval": created.requires_approval,
            "description": created.description,
            "sort_order": created.sort_order,
        }),
        Some(format!("created note kind {key}")),
    )
    .await?;
    txn.commit().await?;

    Ok(created)
}

pub async fn update(
    db: &DatabaseConnection,
    actor: Actor,
    key: &str,
    patch: UpdateNoteKind,
) -> Result<note_kind::Model, AppError> {
    let key = validate_key(key)?;
    let txn = db.begin().await?;
    let current = find_by_key(&txn, &key).await?;
    let mut active = current.clone().into_active_model();
    let mut diff = serde_json::Map::new();

    if let Some(display_name) = patch.display_name {
        let display_name = validate_display_name(&display_name)?;
        diff.insert(
            "display_name".into(),
            json!({ "from": current.display_name, "to": display_name }),
        );
        active.display_name = Set(display_name);
    }
    if let Some(requires_approval) = patch.requires_approval {
        diff.insert(
            "requires_approval".into(),
            json!({ "from": current.requires_approval, "to": requires_approval }),
        );
        active.requires_approval = Set(requires_approval);
    }
    if let Some(description) = patch.description {
        diff.insert(
            "description".into(),
            json!({ "from": current.description, "to": description }),
        );
        active.description = Set(description);
    }
    if let Some(sort_order) = patch.sort_order {
        diff.insert(
            "sort_order".into(),
            json!({ "from": current.sort_order, "to": sort_order }),
        );
        active.sort_order = Set(sort_order);
    }

    if diff.is_empty() {
        txn.rollback().await?;
        return Ok(current);
    }

    let updated = active.update(&txn).await?;
    if current.requires_approval && !updated.requires_approval {
        let note_ids = note::Entity::find()
            .select_only()
            .column(note::Column::Id)
            .filter(note::Column::Kind.eq(&key))
            .into_tuple::<Uuid>()
            .all(&txn)
            .await?;
        let reviewed_at = chrono::Utc::now().fixed_offset();
        for note_id in note_ids {
            let latest_pending = note_version::Entity::find()
                .filter(note_version::Column::NoteId.eq(note_id))
                .filter(note_version::Column::Status.eq("unread"))
                .order_by_desc(note_version::Column::VersionNo)
                .one(&txn)
                .await?;
            let Some(pending) = latest_pending else {
                continue;
            };
            let (approved, previous_current_id) =
                note_versions::approve_pending_version(&txn, note_id, pending, reviewed_at).await?;
            change_history::record_as(
                &txn,
                actor,
                TargetKind::Note,
                note_id,
                Op::StatusChange,
                json!({
                    "from": "unread",
                    "to": "approved",
                    "version_id": approved.id,
                    "previous_current_version_id": previous_current_id,
                    "label": null,
                }),
                None,
            )
            .await?;
        }
    }
    change_history::record_as(
        &txn,
        actor,
        TargetKind::NoteKind,
        history_target_id(&key),
        Op::Update,
        serde_json::Value::Object(diff),
        Some(format!("updated note kind {key}")),
    )
    .await?;
    txn.commit().await?;

    Ok(updated)
}

pub async fn delete(db: &DatabaseConnection, actor: Actor, key: &str) -> Result<(), AppError> {
    let key = validate_key(key)?;
    let txn = db.begin().await?;
    let _current = find_by_key(&txn, &key).await?;

    if note::Entity::find()
        .filter(note::Column::Kind.eq(&key))
        .one(&txn)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(format!(
            "note kind {key} is used by existing notes"
        )));
    }

    let result = note_kind::Entity::delete_by_id(key.clone())
        .exec(&txn)
        .await?;
    if result.rows_affected == 0 {
        return Err(AppError::NotFound(format!("note kind {key} not found")));
    }
    change_history::record_as(
        &txn,
        actor,
        TargetKind::NoteKind,
        history_target_id(&key),
        Op::Delete,
        json!({ "key": key }),
        Some(format!("deleted note kind {key}")),
    )
    .await?;
    txn.commit().await?;
    Ok(())
}
