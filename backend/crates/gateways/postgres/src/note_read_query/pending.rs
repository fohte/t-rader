use core_application::note::{NoteReadQueryError, PendingNoteVersion};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::DatabaseHandle;
use crate::entities::{note, note_version};

use super::{PENDING_STATUS, query_error, to_version};

pub(super) async fn list_pending_note_versions(
    db: &DatabaseHandle,
) -> Result<Vec<PendingNoteVersion>, NoteReadQueryError> {
    note_version::Entity::find()
        .find_also_related(note::Entity)
        .filter(note_version::Column::Status.eq(PENDING_STATUS))
        .order_by_asc(note_version::Column::CreatedAt)
        .order_by_asc(note_version::Column::NoteId)
        .order_by_asc(note_version::Column::VersionNo)
        .all(db)
        .await
        .map_err(query_error)?
        .into_iter()
        .map(|(version, note)| {
            let kind = note.map(|note| note.kind).ok_or_else(|| {
                NoteReadQueryError::InvalidData(format!(
                    "note version {} references missing note {}",
                    version.id, version.note_id
                ))
            })?;
            Ok(PendingNoteVersion {
                version: to_version(version),
                kind,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset};
    use core_application::note::{NoteReadQuery, NoteVersion, PendingNoteVersion};
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::EntityTrait;
    use uuid::Uuid;

    use super::super::PostgresNoteReadQuery;
    use crate::DatabaseHandle;
    use crate::entities::{note, note_kind, note_version};

    #[backend_test_macros::database_test]
    async fn list_pending_versions_includes_note_kind_and_frontmatter(db: DatabaseHandle) {
        let kind = insert_note_kind(&db).await;
        let first_note_id = insert_note(&db, Some(kind.clone())).await;
        let first_version = version_fixture(
            Uuid::new_v4(),
            first_note_id,
            "unread",
            serde_json::json!({ "tags": ["sample-tag", "another-tag"] }),
            timestamp(2),
        );
        insert_version(&db, &first_version).await;

        let ignored_note_id = insert_note(&db, Some(kind.clone())).await;
        let ignored_version = version_fixture(
            Uuid::new_v4(),
            ignored_note_id,
            "approved",
            serde_json::json!({ "tags": ["ignored-tag"] }),
            timestamp(3),
        );
        insert_version(&db, &ignored_version).await;

        let second_note_id = insert_note(&db, None).await;
        let second_version = version_fixture(
            Uuid::new_v4(),
            second_note_id,
            "unread",
            serde_json::json!({}),
            timestamp(4),
        );
        insert_version(&db, &second_version).await;

        let query = PostgresNoteReadQuery::new(db);
        let actual = query
            .list_pending_note_versions()
            .await
            .expect("list pending note versions");

        assert_eq!(
            actual,
            vec![
                PendingNoteVersion {
                    version: first_version,
                    kind: Some(kind),
                },
                PendingNoteVersion {
                    version: second_version,
                    kind: None,
                },
            ],
        );
    }

    async fn insert_note_kind(db: &DatabaseHandle) -> String {
        let key = format!("sample-kind-{}", Uuid::new_v4().simple());
        note_kind::Entity::insert(note_kind::ActiveModel {
            key: Set(key.clone()),
            display_name: Set("Sample kind".to_owned()),
            requires_approval: Set(true),
            description: Set(None),
            sort_order: Set(0),
        })
        .exec_without_returning(db)
        .await
        .expect("insert test note kind");
        key
    }

    async fn insert_note(db: &DatabaseHandle, kind: Option<String>) -> Uuid {
        let id = Uuid::new_v4();
        note::Entity::insert(note::ActiveModel {
            id: Set(id),
            kind: Set(kind),
            trigger: Set(None),
            trigger_label: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
            execution_id: Set(None),
        })
        .exec_without_returning(db)
        .await
        .expect("insert test note");
        id
    }

    async fn insert_version(db: &DatabaseHandle, version: &NoteVersion) {
        note_version::Entity::insert(note_version::ActiveModel {
            id: Set(version.id),
            note_id: Set(version.note_id),
            version_no: Set(version.version_no),
            title: Set(version.title.clone()),
            body_md: Set(version.body_md.clone()),
            frontmatter_json: Set(version.frontmatter_json.clone()),
            graphs_json: Set(version.graphs_json.clone()),
            resolved_price_references_json: Set(version.resolved_price_references_json.clone()),
            status: Set(version.status.clone()),
            is_current: Set(version.is_current),
            change_reason: Set(version.change_reason.clone()),
            created_by_kind: Set(version.created_by_kind.clone()),
            execution_id: Set(version.execution_id.clone()),
            created_at: Set(version.created_at),
            reviewed_at: Set(version.reviewed_at),
        })
        .exec_without_returning(db)
        .await
        .expect("insert test note version");
    }

    fn version_fixture(
        id: Uuid,
        note_id: Uuid,
        status: &str,
        frontmatter_json: serde_json::Value,
        created_at: DateTime<FixedOffset>,
    ) -> NoteVersion {
        NoteVersion {
            id,
            note_id,
            version_no: 1,
            title: "sample note".to_owned(),
            body_md: "sample body".to_owned(),
            frontmatter_json,
            graphs_json: serde_json::json!([]),
            resolved_price_references_json: serde_json::json!({}),
            status: status.to_owned(),
            is_current: false,
            change_reason: Some("sample change".to_owned()),
            created_by_kind: "llm".to_owned(),
            execution_id: None,
            created_at,
            reviewed_at: None,
        }
    }

    fn timestamp(day: u8) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(&format!("2026-10-{day:02}T00:00:00+00:00"))
            .expect("valid test timestamp")
    }
}
