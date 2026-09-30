use std::collections::HashMap;

use core_application::note::{NoteLink, NoteLinkView, NoteLinks, NoteReadQueryError};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect, QueryTrait};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::{note_link, note_version};

use super::{find_current_versions, query_error};

pub(super) async fn find_links_from_version(
    db: &DatabaseHandle,
    version_id: Uuid,
) -> Result<Vec<NoteLink>, NoteReadQueryError> {
    note_link::Entity::find()
        .filter(note_link::Column::FromVersionId.eq(version_id))
        .all(db)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|row| NoteLink {
                    from_version_id: row.from_version_id,
                    to_note_id: row.to_note_id,
                    to_version_id: row.to_version_id,
                })
                .collect()
        })
        .map_err(query_error)
}

pub(super) async fn list_note_links(
    db: &DatabaseHandle,
    note_id: Uuid,
    source_version_id: Uuid,
) -> Result<NoteLinks, NoteReadQueryError> {
    let outgoing_links = note_link::Entity::find()
        .filter(note_link::Column::FromVersionId.eq(source_version_id))
        .all(db)
        .await
        .map_err(query_error)?;
    let target_ids = outgoing_links
        .iter()
        .map(|link| link.to_note_id)
        .collect::<Vec<_>>();
    let current_targets = find_current_versions(db, &target_ids).await?;
    let pinned_ids = outgoing_links
        .iter()
        .filter_map(|link| link.to_version_id)
        .collect::<Vec<_>>();
    let pinned_versions = if pinned_ids.is_empty() {
        Vec::new()
    } else {
        note_version::Entity::find()
            .filter(note_version::Column::Id.is_in(pinned_ids))
            .all(db)
            .await
            .map_err(query_error)?
    };
    let pinned_by_id = pinned_versions
        .into_iter()
        .map(|version| (version.id, version))
        .collect::<HashMap<_, _>>();
    let mut outgoing = outgoing_links
        .into_iter()
        .map(|link| {
            let resolved = match link.to_version_id {
                Some(version_id) => pinned_by_id.get(&version_id),
                None => current_targets.get(&link.to_note_id),
            };
            NoteLinkView {
                note_id: link.to_note_id,
                version_id: link.to_version_id,
                version_no: resolved.map(|version| version.version_no),
                title: resolved.map(|version| version.title.clone()),
            }
        })
        .collect::<Vec<_>>();
    outgoing.sort_by_key(|link| link.note_id);

    let current_version_ids = note_version::Entity::find()
        .select_only()
        .column(note_version::Column::Id)
        .filter(note_version::Column::IsCurrent.eq(true))
        .into_query();
    let incoming_links = note_link::Entity::find()
        .filter(note_link::Column::ToNoteId.eq(note_id))
        .filter(note_link::Column::FromVersionId.in_subquery(current_version_ids))
        .all(db)
        .await
        .map_err(query_error)?;
    let incoming_ids = incoming_links
        .iter()
        .map(|link| link.from_version_id)
        .collect::<Vec<_>>();
    let source_versions = if incoming_ids.is_empty() {
        Vec::new()
    } else {
        note_version::Entity::find()
            .filter(note_version::Column::Id.is_in(incoming_ids))
            .filter(note_version::Column::IsCurrent.eq(true))
            .all(db)
            .await
            .map_err(query_error)?
    };
    let source_by_id = source_versions
        .into_iter()
        .map(|version| (version.id, version))
        .collect::<HashMap<_, _>>();
    let mut incoming = incoming_links
        .into_iter()
        .filter_map(|link| source_by_id.get(&link.from_version_id))
        .map(|version| NoteLinkView {
            note_id: version.note_id,
            version_id: Some(version.id),
            version_no: Some(version.version_no),
            title: Some(version.title.clone()),
        })
        .collect::<Vec<_>>();
    incoming.sort_by_key(|link| link.note_id);
    Ok(NoteLinks { outgoing, incoming })
}
