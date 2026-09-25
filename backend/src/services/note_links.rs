use std::collections::{BTreeMap, HashSet};

use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect, QueryTrait};
use uuid::Uuid;

use crate::entities::{note, note_link, note_version};
use crate::error::AppError;
use crate::services::note_refs::extract_note_link_tokens;
use crate::services::note_versions::find_current_versions;

/// 新しいノートバージョンの作成時点でリンク先の現行バージョンを解決する。
pub async fn sync_note_links<C: sea_orm::ConnectionTrait>(
    db: &C,
    source_note: &note::Model,
    source_version_id: Uuid,
    body_md: &str,
) -> Result<(), AppError> {
    let mut target_policies = BTreeMap::new();
    let mut conflicted_targets = HashSet::new();
    for token in extract_note_link_tokens(body_md) {
        if let Some(previous) = target_policies.insert(token.note_id, token.follows_current)
            && previous != token.follows_current
        {
            conflicted_targets.insert(token.note_id);
        }
    }

    if !conflicted_targets.is_empty() {
        return Err(AppError::Validation(
            "同じノートへのリンクでは固定指定と @current 指定を混在できません".into(),
        ));
    }

    let target_ids: Vec<Uuid> = target_policies
        .keys()
        .copied()
        .filter(|id| !conflicted_targets.contains(id))
        .collect();
    if target_ids.is_empty() {
        return Ok(());
    }

    let targets = note::Entity::find()
        .filter(note::Column::Id.is_in(target_ids.iter().copied()))
        .all(db)
        .await?;
    let targets_by_id: BTreeMap<Uuid, note::Model> = targets
        .into_iter()
        .map(|target| (target.id, target))
        .collect();
    let current_versions = find_current_versions(db, &target_ids).await?;

    let mut links = Vec::with_capacity(target_ids.len());
    for target_id in target_ids {
        let follows_current = target_policies[&target_id];
        let Some(target) = targets_by_id.get(&target_id) else {
            return Err(AppError::Validation(format!(
                "参照先のノート {target_id} が存在しません"
            )));
        };
        if source_note.strategy_id != target.strategy_id {
            return Err(AppError::Validation(
                "参照先のノートは同じ戦略に属している必要があります".into(),
            ));
        }

        let to_version_id = if follows_current {
            None
        } else if let Some(version) = current_versions.get(&target_id) {
            Some(version.id)
        } else {
            return Err(AppError::Validation(format!(
                "参照先のノート {target_id} に現行バージョンがありません"
            )));
        };

        links.push(note_link::ActiveModel {
            from_version_id: Set(source_version_id),
            to_note_id: Set(target_id),
            to_version_id: Set(to_version_id),
        });
    }

    if !links.is_empty() {
        note_link::Entity::insert_many(links)
            .exec_without_returning(db)
            .await?;
    }
    Ok(())
}

pub async fn copy_note_links<C: sea_orm::ConnectionTrait>(
    db: &C,
    source_version_id: Uuid,
    new_source_version_id: Uuid,
) -> Result<(), AppError> {
    let links = find_links_from_version(db, source_version_id).await?;
    if links.is_empty() {
        return Ok(());
    }

    note_link::Entity::insert_many(links.into_iter().map(|link| note_link::ActiveModel {
        from_version_id: Set(new_source_version_id),
        to_note_id: Set(link.to_note_id),
        to_version_id: Set(link.to_version_id),
    }))
    .exec_without_returning(db)
    .await?;
    Ok(())
}

pub async fn find_links_from_version<C: sea_orm::ConnectionTrait>(
    db: &C,
    version_id: Uuid,
) -> Result<Vec<note_link::Model>, sea_orm::DbErr> {
    note_link::Entity::find()
        .filter(note_link::Column::FromVersionId.eq(version_id))
        .all(db)
        .await
}

pub async fn find_current_links_to_note<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: Uuid,
) -> Result<Vec<note_link::Model>, sea_orm::DbErr> {
    let current_version_ids = note_version::Entity::find()
        .select_only()
        .column(note_version::Column::Id)
        .filter(note_version::Column::IsCurrent.eq(true))
        .into_query();

    note_link::Entity::find()
        .filter(note_link::Column::ToNoteId.eq(note_id))
        .filter(note_link::Column::FromVersionId.in_subquery(current_version_ids))
        .all(db)
        .await
}
