//! ノート種別の CRUD を REST と管理 MCP で共有する。

use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    IntoActiveModel, QueryFilter, QueryOrder, SqlErr, TransactionSession, TransactionTrait,
};
use serde_json::json;
use uuid::Uuid;

use crate::entities::{note, note_kind};
use crate::error::AppError;
use crate::services::change_history::{self, Actor, Op, TargetKind};

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

pub async fn create<C>(
    db: &C,
    actor: Actor,
    input: CreateNoteKind,
) -> Result<note_kind::Model, AppError>
where
    C: TransactionTrait,
{
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
        .filter(note::Column::TypeTag.eq(&key))
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

#[cfg(test)]
mod tests {
    use sea_orm::{EntityTrait, QueryFilter};

    use super::*;
    use crate::entities::change_history;
    use crate::testing::connect_with_application_name;

    #[tokio::test]
    #[ignore = "一時的な nested transaction の計測 PoC"]
    // 削除条件: nested transaction の挙動計測が完了したら削除する。
    async fn nested_create_commit_is_rolled_back_by_outer_transaction_refactoring() {
        let db = connect_with_application_name("h8-nested-primary").await;
        let outer = db.begin().await.expect("begin outer transaction");
        let key = "h8-transaction-probe";
        let expected = note_kind::Model {
            key: key.to_string(),
            display_name: "Transaction probe".to_string(),
            requires_approval: true,
            description: Some("Nested commit rollback probe".to_string()),
            sort_order: 41,
        };

        let created = create(
            &outer,
            Actor::Human,
            CreateNoteKind {
                key: key.to_string(),
                display_name: "Transaction probe".to_string(),
                requires_approval: true,
                description: Some("Nested commit rollback probe".to_string()),
                sort_order: Some(41),
            },
        )
        .await
        .expect("create note kind");
        let visible = note_kind::Entity::find_by_id(key.to_string())
            .one(&outer)
            .await
            .expect("find note kind in outer transaction");
        assert_eq!((created, visible), (expected.clone(), Some(expected)));

        outer.rollback().await.expect("rollback outer transaction");
        db.close().await.expect("close primary connection");

        let verifier = connect_with_application_name("h8-nested-verify").await;
        let remaining_note_kind = note_kind::Entity::find_by_id(key.to_string())
            .one(&verifier)
            .await
            .expect("verify note kind rollback");
        let remaining_history = change_history::Entity::find()
            .filter(change_history::Column::TargetKind.eq("note_kind"))
            .filter(change_history::Column::TargetId.eq(history_target_id(key)))
            .all(&verifier)
            .await
            .expect("verify change history rollback");
        assert_eq!((remaining_note_kind, remaining_history), (None, Vec::new()));
        verifier.close().await.expect("close verifier connection");
    }
}
