//! note version の読み取りと note_kind 更新時の承認処理を提供する。

#[cfg(test)]
use sea_orm::ActiveModelTrait;
#[cfg(test)]
use sea_orm::ActiveValue::NotSet;
#[cfg(test)]
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
#[cfg(test)]
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

#[cfg(test)]
use crate::error::AppError;
#[cfg(test)]
use crate::services::change_history::{self, Actor, Op, TargetKind};
#[cfg(test)]
use crate::services::note_links::{copy_note_links, sync_note_links};
#[cfg(test)]
use crate::services::note_refs::sync_note_refs;
#[cfg(test)]
use crate::services::note_refs::sync_note_refs_after_graphs_only_update;
#[cfg(test)]
use gateway_postgres::entities::note;
#[cfg(test)]
use gateway_postgres::entities::note_kind;
use gateway_postgres::entities::note_version;

#[cfg(test)]
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
#[cfg(test)]
const APPROVED_NOTE_STATUS: &str = "approved";
#[cfg(test)]
const HUMAN_CREATED_BY_KIND: &str = "human";

/// 新しいバージョンを追加し、種別の承認設定に応じて現行バージョンを切り替える。
///
/// 人間が追加したバージョンは承認済みで現行にする。承認必須の種別に対してエージェントが
/// 追加したバージョンは、承認されるまで現行バージョンを維持する。
#[cfg(test)]
pub async fn append_version(
    txn: &impl sea_orm::ConnectionTrait,
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

#[cfg(test)]
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
