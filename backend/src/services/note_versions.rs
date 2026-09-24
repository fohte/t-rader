//! ノートの追記専用バージョンを作成し、現行版に付随するデータを同期する。

use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QuerySelect,
};
use serde_json::json;
use uuid::Uuid;

use crate::entities::{note, note_version};
use crate::error::AppError;
use crate::services::change_history::{self, Actor, Op, TargetKind};
use crate::services::comment_anchor;
use crate::services::note_refs::{sync_note_refs, sync_note_refs_after_graphs_only_update};

pub struct AppendVersion {
    pub title: String,
    pub body_md: String,
    pub frontmatter_json: serde_json::Value,
    pub graphs_json: serde_json::Value,
    pub created_by_kind: String,
    pub execution_id: Option<String>,
    pub change_reason: Option<String>,
    pub change_diff: Option<serde_json::Value>,
}

/// 新しい版を追加し、その版を現行にする。
///
/// `status` は入力に含めず、追加した版は必ず unread から始める。
pub async fn append_version(
    txn: &DatabaseTransaction,
    note_id: Uuid,
    content: AppendVersion,
) -> Result<note_version::Model, AppError> {
    let previous = find_current_version(txn, note_id).await?;
    let existing_version_numbers = note_version::Entity::find()
        .select_only()
        .column(note_version::Column::VersionNo)
        .filter(note_version::Column::NoteId.eq(note_id))
        .into_tuple::<i32>()
        .all(txn)
        .await?;
    let version_no = existing_version_numbers
        .into_iter()
        .max()
        .unwrap_or_default()
        .checked_add(1)
        .ok_or_else(|| AppError::Validation("note version number overflow".into()))?;

    if let Some(previous) = previous.as_ref() {
        note_version::ActiveModel {
            id: Set(previous.id),
            is_current: Set(false),
            ..Default::default()
        }
        .update(txn)
        .await?;
    }

    let version = note_version::Entity::insert(note_version::ActiveModel {
        id: Set(Uuid::new_v4()),
        note_id: Set(note_id),
        version_no: Set(version_no),
        title: Set(content.title),
        body_md: Set(content.body_md),
        frontmatter_json: Set(content.frontmatter_json),
        graphs_json: Set(content.graphs_json),
        status: Set("unread".into()),
        is_current: Set(true),
        change_reason: Set(content.change_reason.clone()),
        created_by_kind: Set(content.created_by_kind.clone()),
        execution_id: Set(content.execution_id),
        created_at: NotSet,
        reviewed_at: Set(None),
    })
    .exec_with_returning(txn)
    .await?;

    note::ActiveModel {
        id: Set(note_id),
        updated_at: Set(chrono::Utc::now().fixed_offset()),
        ..Default::default()
    }
    .update(txn)
    .await?;

    let body_changed = previous
        .as_ref()
        .is_none_or(|previous| previous.body_md != version.body_md);
    let graphs_changed = previous
        .as_ref()
        .is_none_or(|previous| previous.graphs_json != version.graphs_json);
    if body_changed || graphs_changed {
        if body_changed {
            sync_note_refs(txn, note_id, &version.body_md, &version.graphs_json).await?;
        } else {
            sync_note_refs_after_graphs_only_update(
                txn,
                note_id,
                &version.body_md,
                &version.graphs_json,
            )
            .await?;
        }
    }
    if body_changed {
        comment_anchor::reanchor_note_comments(txn, note_id, &version.body_md).await?;
    }

    let actor = match content.created_by_kind.as_str() {
        "human" => Actor::Human,
        "llm" => Actor::Llm {
            label: "strategy-mcp",
        },
        other => {
            return Err(AppError::Validation(format!(
                "invalid created_by_kind: {other}"
            )));
        }
    };
    let mut diff = json!({
        "from_version_id": previous.as_ref().map(|version| version.id),
        "to_version_id": version.id,
        "version_no": version.version_no,
    });
    if let Some(change_diff) = content.change_diff
        && let Some(changes) = change_diff.as_object()
    {
        for (key, value) in changes {
            diff[key] = value.clone();
        }
    }
    change_history::record_as(
        txn,
        actor,
        TargetKind::Note,
        note_id,
        if previous.is_some() {
            Op::Update
        } else {
            Op::Create
        },
        diff,
        content.change_reason,
    )
    .await?;

    Ok(version)
}

pub async fn find_current_version<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: Uuid,
) -> Result<Option<note_version::Model>, sea_orm::DbErr> {
    note_version::Entity::find()
        .filter(note_version::Column::NoteId.eq(note_id))
        .filter(note_version::Column::IsCurrent.eq(true))
        .one(db)
        .await
}

pub async fn find_current_versions<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_ids: &[Uuid],
) -> Result<Vec<note_version::Model>, sea_orm::DbErr> {
    if note_ids.is_empty() {
        return Ok(Vec::new());
    }

    note_version::Entity::find()
        .filter(note_version::Column::NoteId.is_in(note_ids.iter().copied()))
        .filter(note_version::Column::IsCurrent.eq(true))
        .all(db)
        .await
}

pub async fn note_ids_with_current_status<C: sea_orm::ConnectionTrait>(
    db: &C,
    status: &str,
) -> Result<Vec<Uuid>, sea_orm::DbErr> {
    note_version::Entity::find()
        .select_only()
        .column(note_version::Column::NoteId)
        .filter(note_version::Column::IsCurrent.eq(true))
        .filter(note_version::Column::Status.eq(status))
        .into_tuple::<Uuid>()
        .all(db)
        .await
}
