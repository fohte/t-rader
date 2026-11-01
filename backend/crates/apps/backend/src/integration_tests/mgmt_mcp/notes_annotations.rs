use super::dto::*;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::testing::insert_test_note;
    use chrono::{DateTime, FixedOffset};
    use core_application::agent_task_client::FakeAgentTaskClient;
    use gateway_postgres::entities::{annotation, note, note_version};
    use rmcp::handler::server::wrapper::{Json, Parameters};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use serde_json::json;
    use uuid::Uuid;

    use super::super::dto::{AnnotationMeta, ListRecentAnnotationsResult};
    use super::super::tests_common::build_server;
    use super::*;

    fn test_timestamp(value: &str) -> DateTime<FixedOffset> {
        value.parse().expect("timestamp")
    }

    async fn seed_annotation(
        db: &gateway_postgres::DatabaseHandle,
        target_symbol: &str,
        target_kind: &str,
        status: &str,
        created_by_kind: &str,
        updated_at: DateTime<FixedOffset>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        annotation::ActiveModel {
            id: Set(id),
            target_symbol: Set(target_symbol.into()),
            target_kind: Set(target_kind.into()),
            timestamp: Set(updated_at),
            timestamp_start: Set(None),
            price: Set(None),
            price_start: Set(None),
            text: Set("sample text".into()),
            status: Set(status.into()),
            linked_note_id: Set(None),
            created_by_kind: Set(created_by_kind.into()),
            created_at: Set(updated_at),
            updated_at: Set(updated_at),
            execution_step_id: Set(None),
            execution_task_id: Set(None),
        }
        .insert(db)
        .await
        .expect("insert annotation");
        id
    }

    async fn seed_unread_version(
        db: &gateway_postgres::DatabaseHandle,
        note_id: Uuid,
        version_no: i32,
        title: &str,
    ) {
        note_version::ActiveModel {
            id: Set(Uuid::new_v4()),
            note_id: Set(note_id),
            version_no: Set(version_no),
            title: Set(title.into()),
            body_md: Set("sample body".into()),
            frontmatter_json: Set(json!({})),
            graphs_json: Set(json!([])),
            resolved_price_references_json: Set(json!({})),
            warning_dispositions_json: Set(json!([{
                "warning_id": "sample-warning",
                "value": "sample-value",
                "location": { "kind": "body", "start": 0, "end": 1 },
                "reason": "sample reason"
            }])),
            status: Set("unread".into()),
            is_current: Set(false),
            change_reason: Set(None),
            created_by_kind: Set("llm".into()),
            execution_id: Set(None),
            created_at: NotSet,
            reviewed_at: Set(None),
        }
        .insert(db)
        .await
        .expect("insert unread version");
    }

    #[backend_test_macros::database_test]
    async fn list_recent_notes_caps_by_limit(db: gateway_postgres::DatabaseHandle) {
        for i in 0..5 {
            insert_test_note(&db, &format!("note-{i}"), "body").await;
        }
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));
        let Json(result) = server
            .list_recent_notes(Parameters(ListRecentParams { limit: Some(3) }))
            .await
            .expect("ok");
        assert_eq!(result.notes.len(), 3);
    }

    #[backend_test_macros::database_test]
    async fn list_recent_notes_includes_pending_versions_with_review_state(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let older_updated_at = test_timestamp("2026-07-01T00:00:00Z");
        let newer_updated_at = test_timestamp("2026-07-02T00:00:00Z");
        let pending_only_id = insert_test_note(&db, "Pending only sample", "sample body").await;
        let pending_only_version = crate::testing::find_current_note_version(&db, pending_only_id)
            .await
            .expect("find pending-only version")
            .expect("pending-only version exists");
        note_version::ActiveModel {
            id: Set(pending_only_version.id),
            status: Set("unread".into()),
            is_current: Set(false),
            created_by_kind: Set("llm".into()),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("make version pending");
        note::ActiveModel {
            id: Set(pending_only_id),
            updated_at: Set(older_updated_at),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("set pending-only update time");

        let mixed_id = insert_test_note(&db, "Current sample", "sample body").await;
        seed_unread_version(&db, mixed_id, 2, "Pending sample").await;
        note::ActiveModel {
            id: Set(mixed_id),
            updated_at: Set(newer_updated_at),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("set mixed note update time");

        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));
        let Json(result) = server
            .list_recent_notes(Parameters(ListRecentParams { limit: Some(10) }))
            .await
            .expect("list recent notes");

        assert_eq!(
            serde_json::to_value(result).expect("serialize result"),
            serde_json::to_value(ListRecentNotesResult {
                notes: vec![
                    NoteMeta {
                        note_id: mixed_id,
                        title: "Current sample".into(),
                        status: "approved".into(),
                        created_by_kind: "human".into(),
                        updated_at: newer_updated_at,
                        is_current: true,
                        unread_version_count: 1,
                    },
                    NoteMeta {
                        note_id: pending_only_id,
                        title: "Pending only sample".into(),
                        status: "unread".into(),
                        created_by_kind: "llm".into(),
                        updated_at: older_updated_at,
                        is_current: false,
                        unread_version_count: 1,
                    },
                ],
            })
            .expect("serialize expected result"),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_recent_annotations_returns_global_results_newest_first(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let older_updated_at = test_timestamp("2026-07-01T00:00:00Z");
        let newer_updated_at = test_timestamp("2026-07-02T00:00:00Z");
        let foreign_updated_at = test_timestamp("2026-07-03T00:00:00Z");
        let older_id = seed_annotation(
            &db,
            "SAMPLE-A",
            "sample-kind-a",
            "unread",
            "human",
            older_updated_at,
        )
        .await;
        let newer_id = seed_annotation(
            &db,
            "SAMPLE-B",
            "sample-kind-b",
            "approved",
            "llm",
            newer_updated_at,
        )
        .await;
        let latest_id = seed_annotation(
            &db,
            "FOREIGN",
            "foreign-kind",
            "rejected",
            "human",
            foreign_updated_at,
        )
        .await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .list_recent_annotations(Parameters(ListRecentParams { limit: Some(3) }))
            .await
            .expect("list recent annotations");

        assert_eq!(
            serde_json::to_value(result).expect("serialize result"),
            serde_json::to_value(ListRecentAnnotationsResult {
                annotations: vec![
                    AnnotationMeta {
                        annotation_id: latest_id,
                        target_symbol: "FOREIGN".into(),
                        target_kind: "foreign-kind".into(),
                        status: "rejected".into(),
                        created_by_kind: "human".into(),
                        updated_at: foreign_updated_at,
                    },
                    AnnotationMeta {
                        annotation_id: newer_id,
                        target_symbol: "SAMPLE-B".into(),
                        target_kind: "sample-kind-b".into(),
                        status: "approved".into(),
                        created_by_kind: "llm".into(),
                        updated_at: newer_updated_at,
                    },
                    AnnotationMeta {
                        annotation_id: older_id,
                        target_symbol: "SAMPLE-A".into(),
                        target_kind: "sample-kind-a".into(),
                        status: "unread".into(),
                        created_by_kind: "human".into(),
                        updated_at: older_updated_at,
                    },
                ],
            })
            .expect("serialize expected result"),
        );
    }
}
