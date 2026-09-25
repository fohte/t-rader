//! ノートの追記専用バージョンを作成し、現行版に付随するデータを同期する。

use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QuerySelect,
    QueryTrait,
};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

use crate::entities::{note, note_version};
use crate::error::AppError;
use crate::services::change_history::{self, Actor, Op, TargetKind};
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
    pub actor: Actor,
}

pub const INITIAL_NOTE_STATUS: &str = "unread";

/// 新しい版を追加し、その版を現行にする。
///
/// `status` は入力に含めず、追加した版は必ず unread から始める。
pub async fn append_version(
    txn: &DatabaseTransaction,
    note_id: Uuid,
    content: AppendVersion,
) -> Result<note_version::Model, AppError> {
    let previous = find_current_version(txn, note_id).await?;
    let max_version_no = note_version::Entity::find()
        .select_only()
        .column_as(note_version::Column::VersionNo.max(), "max_version_no")
        .filter(note_version::Column::NoteId.eq(note_id))
        .into_tuple::<Option<i32>>()
        .one(txn)
        .await?;
    let version_no = max_version_no
        .flatten()
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
        status: Set(INITIAL_NOTE_STATUS.into()),
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
        content.actor,
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

/// 指定した版を現行にし、本文に紐づく参照データを同期する。
pub async fn set_current_version(
    txn: &DatabaseTransaction,
    note_id: Uuid,
    version: note_version::Model,
    new_status: Option<&str>,
    reviewed_at: Option<chrono::DateTime<chrono::FixedOffset>>,
) -> Result<(note_version::Model, Option<Uuid>), AppError> {
    let current = find_current_version(txn, note_id).await?;
    let previous_current_id = current.as_ref().map(|current| current.id);
    if let Some(current) = current.as_ref().filter(|current| current.id != version.id) {
        note_version::ActiveModel {
            id: Set(current.id),
            is_current: Set(false),
            ..Default::default()
        }
        .update(txn)
        .await?;
    }

    let mut active = note_version::ActiveModel {
        id: Set(version.id),
        is_current: Set(true),
        ..Default::default()
    };
    if let Some(status) = new_status {
        active.status = Set(status.to_string());
    }
    if let Some(reviewed_at) = reviewed_at {
        active.reviewed_at = Set(Some(reviewed_at));
    }
    let updated_version = active.update(txn).await?;

    let note_row = note::Entity::find_by_id(note_id)
        .one(txn)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("note {note_id} not found")))?;
    note::ActiveModel {
        id: Set(note_id),
        updated_at: Set(chrono::Utc::now().fixed_offset()),
        ..Default::default()
    }
    .update(txn)
    .await?;

    let content_changed = current.as_ref().is_none_or(|current| {
        current.body_md != updated_version.body_md
            || current.graphs_json != updated_version.graphs_json
    });
    if content_changed {
        sync_note_refs(
            txn,
            note_row.id,
            &updated_version.body_md,
            &updated_version.graphs_json,
        )
        .await?;
    }

    Ok((updated_version, previous_current_id))
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
) -> Result<HashMap<Uuid, note_version::Model>, sea_orm::DbErr> {
    if note_ids.is_empty() {
        return Ok(HashMap::new());
    }

    Ok(note_version::Entity::find()
        .filter(note_version::Column::NoteId.is_in(note_ids.iter().copied()))
        .filter(note_version::Column::IsCurrent.eq(true))
        .all(db)
        .await?
        .into_iter()
        .map(|version| (version.note_id, version))
        .collect())
}

pub async fn find_initial_created_by_kind<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_ids: &[Uuid],
) -> Result<HashMap<Uuid, String>, sea_orm::DbErr> {
    if note_ids.is_empty() {
        return Ok(HashMap::new());
    }

    Ok(note_version::Entity::find()
        .filter(note_version::Column::NoteId.is_in(note_ids.iter().copied()))
        .filter(note_version::Column::VersionNo.eq(1))
        .all(db)
        .await?
        .into_iter()
        .map(|version| (version.note_id, version.created_by_kind))
        .collect())
}

pub fn current_note_ids_with_status(status: &str) -> sea_orm::sea_query::SelectStatement {
    note_version::Entity::find()
        .select_only()
        .column(note_version::Column::NoteId)
        .filter(note_version::Column::IsCurrent.eq(true))
        .filter(note_version::Column::Status.eq(status))
        .into_query()
}
