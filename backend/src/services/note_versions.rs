//! ノートの追記専用バージョンを作成し、現行版に付随するデータを同期する。

use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait,
};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

use crate::entities::{note, note_kind, note_version};
use crate::error::AppError;
use crate::services::change_history::{self, Actor, Op, TargetKind};
use crate::services::note_links::{copy_note_links, sync_note_links};
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
const APPROVED_NOTE_STATUS: &str = "approved";
const HUMAN_CREATED_BY_KIND: &str = "human";

/// 新しい版を追加し、種別の承認設定に応じて現行版を切り替える。
///
/// 人間の版は承認済みで現行にする。承認必須の種別に対するエージェントの版は、
/// 承認されるまで現行版を維持する。
pub async fn append_version(
    txn: &DatabaseTransaction,
    note_id: Uuid,
    content: AppendVersion,
) -> Result<note_version::Model, AppError> {
    let previous = find_current_version(txn, note_id).await?;
    let previous_version = find_latest_version(txn, note_id).await?;
    let note_row = note::Entity::find_by_id(note_id)
        .one(txn)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("note {note_id} not found")))?;
    let requires_approval = if content.created_by_kind == HUMAN_CREATED_BY_KIND {
        false
    } else if let Some(kind_key) = note_row.kind.as_deref() {
        let kind = note_kind::Entity::find_by_id(kind_key.to_string())
            .one(txn)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("note kind {kind_key} not found")))?;
        kind.requires_approval
    } else {
        false
    };
    let max_version_no = previous_version
        .as_ref()
        .map_or(0, |version| version.version_no);
    if requires_approval
        && max_version_no > 0
        && content
            .change_reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty())
    {
        return Err(AppError::Validation(
            "change_reason is required for updates to this note kind".into(),
        ));
    }
    let becomes_current = !requires_approval;
    let status = if content.created_by_kind == HUMAN_CREATED_BY_KIND {
        APPROVED_NOTE_STATUS
    } else {
        INITIAL_NOTE_STATUS
    };
    let version_no = max_version_no
        .checked_add(1)
        .ok_or_else(|| AppError::Validation("note version number overflow".into()))?;

    if becomes_current && let Some(previous) = previous.as_ref() {
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
        status: Set(status.into()),
        is_current: Set(becomes_current),
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

    let previous_content = previous.as_ref().or(previous_version.as_ref());
    let body_changed = previous_content
        .as_ref()
        .is_none_or(|previous| previous.body_md != version.body_md);
    let graphs_changed = previous_content
        .as_ref()
        .is_none_or(|previous| previous.graphs_json != version.graphs_json);
    if body_changed {
        if becomes_current {
            sync_note_refs(txn, note_id, &version.body_md, &version.graphs_json).await?;
        }
        let source_note = note_row;
        sync_note_links(txn, &source_note, version.id, &version.body_md).await?;
    } else {
        if becomes_current && graphs_changed {
            sync_note_refs_after_graphs_only_update(
                txn,
                note_id,
                &version.body_md,
                &version.graphs_json,
            )
            .await?;
        }
        if let Some(previous) = previous_content {
            copy_note_links(txn, previous.id, version.id).await?;
        }
    }
    let mut diff = json!({
        "from_version_id": previous_content.map(|version| version.id),
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
        if max_version_no > 0 {
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

pub async fn approve_pending_version(
    txn: &DatabaseTransaction,
    note_id: Uuid,
    version: note_version::Model,
    reviewed_at: chrono::DateTime<chrono::FixedOffset>,
) -> Result<(note_version::Model, Option<Uuid>), AppError> {
    let current = find_current_version(txn, note_id).await?;
    if current
        .as_ref()
        .is_some_and(|current| current.version_no > version.version_no)
    {
        let current_id = current.map(|current| current.id);
        let updated = note_version::ActiveModel {
            id: Set(version.id),
            status: Set(APPROVED_NOTE_STATUS.into()),
            reviewed_at: Set(Some(reviewed_at)),
            ..Default::default()
        }
        .update(txn)
        .await?;
        note::ActiveModel {
            id: Set(note_id),
            updated_at: Set(reviewed_at),
            ..Default::default()
        }
        .update(txn)
        .await?;
        return Ok((updated, current_id));
    }

    set_current_version(
        txn,
        note_id,
        version,
        Some(APPROVED_NOTE_STATUS),
        Some(reviewed_at),
    )
    .await
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

pub async fn find_latest_version<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: Uuid,
) -> Result<Option<note_version::Model>, sea_orm::DbErr> {
    note_version::Entity::find()
        .filter(note_version::Column::NoteId.eq(note_id))
        .order_by_desc(note_version::Column::VersionNo)
        .one(db)
        .await
}

pub async fn find_current_or_latest_version<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: Uuid,
) -> Result<Option<note_version::Model>, sea_orm::DbErr> {
    match find_current_version(db, note_id).await? {
        Some(version) => Ok(Some(version)),
        None => find_latest_version(db, note_id).await,
    }
}

pub async fn find_version_of_note<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: Uuid,
    version_id: Option<Uuid>,
) -> Result<Option<note_version::Model>, sea_orm::DbErr> {
    if let Some(version_id) = version_id {
        note_version::Entity::find_by_id(version_id)
            .filter(note_version::Column::NoteId.eq(note_id))
            .one(db)
            .await
    } else {
        find_current_version(db, note_id).await
    }
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

pub fn current_note_ids() -> sea_orm::sea_query::SelectStatement {
    note_version::Entity::find()
        .select_only()
        .column(note_version::Column::NoteId)
        .filter(note_version::Column::IsCurrent.eq(true))
        .into_query()
}
