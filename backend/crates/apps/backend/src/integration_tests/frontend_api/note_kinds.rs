#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
    use axum::http::StatusCode;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set, Unchanged};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
    use serde_json::json;
    use uuid::Uuid;

    use gateway_postgres::entities::{change_history, note, note_kind, note_version};

    use crate::testing::{
        create_test_server, insert_test_note_with_execution_id, insert_test_strategy,
    };

    async fn insert_test_version(
        db: &gateway_postgres::DatabaseHandle,
        note_id: Uuid,
        version_no: i32,
        status: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        note_version::Entity::insert(note_version::ActiveModel {
            id: Set(id),
            note_id: Set(note_id),
            version_no: Set(version_no),
            title: Set(format!("version {version_no}")),
            body_md: Set("body".into()),
            frontmatter_json: Set(json!({})),
            graphs_json: Set(json!([])),
            status: Set(status.into()),
            is_current: Set(false),
            change_reason: Set(None),
            created_by_kind: Set("llm".into()),
            execution_id: Set(None),
            created_at: NotSet,
            reviewed_at: Set(None),
        })
        .exec_without_returning(db)
        .await
        .expect("insert test note version");
        id
    }

    #[backend_test_macros::database_test]
    async fn disabling_note_kind_approval_supersedes_old_pending_versions_and_preserves_rejections(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db.clone()).await;
        let kind_key = "sample-kind";
        note_kind::ActiveModel {
            key: Set(kind_key.to_string()),
            display_name: Set("Sample Kind".to_string()),
            requires_approval: Set(true),
            description: Set(None),
            sort_order: Set(0),
        }
        .insert(&db)
        .await
        .expect("insert note kind");
        let strategy_id = insert_test_strategy(&db, "Sample strategy").await;
        let note_id = insert_test_note_with_execution_id(
            &db,
            strategy_id,
            "Sample title",
            "Sample body",
            "sample-execution-id",
        )
        .await;
        note::ActiveModel {
            id: Unchanged(note_id),
            kind: Set(Some(kind_key.to_string())),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("set note kind");
        let initial_version = note_version::Entity::find()
            .filter(note_version::Column::NoteId.eq(note_id))
            .one(&db)
            .await
            .expect("find pending version")
            .expect("pending version exists");
        note_version::ActiveModel {
            id: Unchanged(initial_version.id),
            status: Set("approved".to_string()),
            is_current: Set(true),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("set initial version state");
        let older_pending_id = insert_test_version(&db, note_id, 2, "unread").await;
        insert_test_version(&db, note_id, 3, "rejected").await;
        let approved_version_id = insert_test_version(&db, note_id, 4, "unread").await;

        let response = server
            .patch(&format!("/api/note-kinds/{kind_key}"))
            .json(&json!({ "requires_approval": false }))
            .await;
        assert_response_eq(
            &response,
            StatusCode::OK,
            Some(json!({
                "key": kind_key,
                "display_name": "Sample Kind",
                "requires_approval": false,
                "description": null,
                "sort_order": 0,
            })),
        );

        let versions = note_version::Entity::find()
            .filter(note_version::Column::NoteId.eq(note_id))
            .order_by_asc(note_version::Column::VersionNo)
            .all(&db)
            .await
            .expect("list note versions")
            .into_iter()
            .map(|version| (version.version_no, version.status, version.is_current))
            .collect::<Vec<_>>();
        let note_kind_history_id = Uuid::new_v5(
            &Uuid::NAMESPACE_OID,
            format!("note_kind:{kind_key}").as_bytes(),
        );
        let history = change_history::Entity::find()
            .filter(change_history::Column::TargetId.is_in([note_id, note_kind_history_id]))
            .all(&db)
            .await
            .expect("find change history")
            .into_iter()
            .filter(|record| record.op == "status_change" || record.op == "update")
            .map(|record| {
                (
                    record.target_kind,
                    record.actor_kind,
                    record.actor_label,
                    record.op,
                    record.diff_json,
                    record.summary,
                )
            })
            .collect::<Vec<_>>();
        let mut history = history;
        history.sort_by_key(|record| record.0.clone());

        assert_eq!(
            (versions, history,),
            (
                vec![
                    (1, "approved".to_string(), false),
                    (2, "superseded".to_string(), false),
                    (3, "rejected".to_string(), false),
                    (4, "approved".to_string(), true),
                ],
                vec![
                    (
                        "note".to_string(),
                        "human".to_string(),
                        "user".to_string(),
                        "status_change".to_string(),
                        json!({
                            "from": "unread",
                            "to": "approved",
                            "version_id": approved_version_id,
                            "previous_current_version_id": initial_version.id,
                            "superseded_version_ids": [older_pending_id],
                            "label": null,
                        }),
                        None,
                    ),
                    (
                        "note_kind".to_string(),
                        "human".to_string(),
                        "user".to_string(),
                        "update".to_string(),
                        json!({
                            "requires_approval": { "from": true, "to": false },
                        }),
                        Some(format!("updated note kind {kind_key}")),
                    ),
                ],
            ),
        );
    }
}
