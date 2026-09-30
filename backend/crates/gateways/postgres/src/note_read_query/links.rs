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

#[cfg(test)]
mod tests {
    use core_application::note::{NoteLinkView, NoteLinks};
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::EntityTrait;
    use uuid::Uuid;

    use super::list_note_links;
    use crate::DatabaseHandle;
    use crate::entities::{note, note_link, note_version};

    struct NoteLinksFixture {
        note_id: Uuid,
        source_version_id: Uuid,
        pinned_target_id: Uuid,
        pinned_version_id: Uuid,
        current_target_id: Uuid,
        current_incoming_note_id: Uuid,
        current_incoming_version_id: Uuid,
    }

    impl NoteLinksFixture {
        async fn seed(db: &DatabaseHandle) -> Self {
            let fixture = Self {
                note_id: Uuid::new_v4(),
                source_version_id: Uuid::new_v4(),
                pinned_target_id: Uuid::new_v4(),
                pinned_version_id: Uuid::new_v4(),
                current_target_id: Uuid::new_v4(),
                current_incoming_note_id: Uuid::new_v4(),
                current_incoming_version_id: Uuid::new_v4(),
            };
            let historical_incoming_note_id = Uuid::new_v4();
            let pinned_target_current_version_id = Uuid::new_v4();
            let current_target_old_version_id = Uuid::new_v4();
            let current_target_version_id = Uuid::new_v4();
            let historical_incoming_version_id = Uuid::new_v4();
            let historical_incoming_current_version_id = Uuid::new_v4();

            for note_id in [
                fixture.note_id,
                fixture.pinned_target_id,
                fixture.current_target_id,
                fixture.current_incoming_note_id,
                historical_incoming_note_id,
            ] {
                insert_note(db, note_id).await;
            }

            insert_version(
                db,
                fixture.pinned_target_id,
                fixture.pinned_version_id,
                1,
                "Pinned target version",
                false,
            )
            .await;
            insert_version(
                db,
                fixture.pinned_target_id,
                pinned_target_current_version_id,
                2,
                "Pinned target current version",
                true,
            )
            .await;
            insert_version(
                db,
                fixture.current_target_id,
                current_target_old_version_id,
                1,
                "Old target",
                false,
            )
            .await;
            insert_version(
                db,
                fixture.current_target_id,
                current_target_version_id,
                2,
                "Current target version",
                true,
            )
            .await;
            insert_version(
                db,
                fixture.note_id,
                fixture.source_version_id,
                2,
                "Source current version",
                true,
            )
            .await;
            insert_version(
                db,
                fixture.current_incoming_note_id,
                fixture.current_incoming_version_id,
                1,
                "Current incoming source",
                true,
            )
            .await;
            insert_version(
                db,
                historical_incoming_note_id,
                historical_incoming_version_id,
                1,
                "Historical incoming source",
                false,
            )
            .await;
            insert_version(
                db,
                historical_incoming_note_id,
                historical_incoming_current_version_id,
                2,
                "Historical source current version",
                true,
            )
            .await;

            insert_link(
                db,
                fixture.source_version_id,
                fixture.pinned_target_id,
                Some(fixture.pinned_version_id),
            )
            .await;
            insert_link(
                db,
                fixture.source_version_id,
                fixture.current_target_id,
                None,
            )
            .await;
            insert_link(
                db,
                fixture.current_incoming_version_id,
                fixture.note_id,
                None,
            )
            .await;
            insert_link(db, historical_incoming_version_id, fixture.note_id, None).await;

            fixture
        }
    }

    #[backend_test_macros::database_test]
    async fn list_note_links_resolves_target_versions_and_only_lists_current_incoming_links(
        db: DatabaseHandle,
    ) {
        let fixture = NoteLinksFixture::seed(&db).await;
        let actual = list_note_links(&db, fixture.note_id, fixture.source_version_id)
            .await
            .expect("list note links");

        let mut expected_outgoing = vec![
            NoteLinkView {
                note_id: fixture.pinned_target_id,
                version_id: Some(fixture.pinned_version_id),
                version_no: Some(1),
                title: Some("Pinned target version".to_string()),
            },
            NoteLinkView {
                note_id: fixture.current_target_id,
                version_id: None,
                version_no: Some(2),
                title: Some("Current target version".to_string()),
            },
        ];
        expected_outgoing.sort_by_key(|link| link.note_id);

        assert_eq!(
            actual,
            NoteLinks {
                outgoing: expected_outgoing,
                incoming: vec![NoteLinkView {
                    note_id: fixture.current_incoming_note_id,
                    version_id: Some(fixture.current_incoming_version_id),
                    version_no: Some(1),
                    title: Some("Current incoming source".to_string()),
                }],
            },
        );
    }

    async fn insert_note(db: &DatabaseHandle, note_id: Uuid) {
        note::Entity::insert(note::ActiveModel {
            id: Set(note_id),
            strategy_id: Set(None),
            kind: Set(None),
            trigger: Set(None),
            trigger_label: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
            execution_id: Set(None),
        })
        .exec_without_returning(db)
        .await
        .expect("insert test note");
    }

    async fn insert_version(
        db: &DatabaseHandle,
        note_id: Uuid,
        version_id: Uuid,
        version_no: i32,
        title: &str,
        is_current: bool,
    ) {
        note_version::Entity::insert(note_version::ActiveModel {
            id: Set(version_id),
            note_id: Set(note_id),
            version_no: Set(version_no),
            title: Set(title.to_string()),
            body_md: Set("Sample body".to_string()),
            frontmatter_json: Set(serde_json::json!({})),
            graphs_json: Set(serde_json::json!([])),
            status: Set("approved".to_string()),
            is_current: Set(is_current),
            change_reason: Set(None),
            created_by_kind: Set("human".to_string()),
            execution_id: Set(None),
            created_at: NotSet,
            reviewed_at: Set(None),
        })
        .exec_without_returning(db)
        .await
        .expect("insert test note version");
    }

    async fn insert_link(
        db: &DatabaseHandle,
        from_version_id: Uuid,
        to_note_id: Uuid,
        to_version_id: Option<Uuid>,
    ) {
        note_link::Entity::insert(note_link::ActiveModel {
            from_version_id: Set(from_version_id),
            to_note_id: Set(to_note_id),
            to_version_id: Set(to_version_id),
        })
        .exec_without_returning(db)
        .await
        .expect("insert test note link");
    }
}
