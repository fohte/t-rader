#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, FixedOffset};
    use gateway_postgres::entities::task_execution_payload;
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::testing::{
        create_test_server_with_db, insert_test_strategy, insert_test_strategy_task,
    };

    fn timestamp(seconds: i64) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2025-06-01T00:00:00Z").expect("parse synthetic timestamp")
            + Duration::seconds(seconds)
    }

    fn cursor(created_at: DateTime<FixedOffset>, id: Uuid) -> String {
        format!("{}|{id}", created_at.to_rfc3339())
    }

    fn encode_query_cursor(cursor: &str) -> String {
        cursor.replace('+', "%2B").replace('|', "%7C")
    }

    async fn insert_manifest(
        db: &gateway_postgres::DatabaseHandle,
        task_id: Uuid,
        id: Uuid,
        created_at: DateTime<FixedOffset>,
        status: &str,
        incomplete_reason: Option<&str>,
    ) {
        task_execution_payload::ActiveModel {
            id: Set(id),
            task_id: Set(task_id),
            execution_step_id: Set(None),
            kind: Set("input".into()),
            source: Set("synthetic-source".into()),
            source_revision: Set(Some("synthetic-revision".into())),
            source_locator: Set(Some(json!({"record": "synthetic"}))),
            source_metadata: Set(json!({"origin": "synthetic"})),
            schema: Set(json!({"type": "array"})),
            root_is_array: Set(true),
            chunk_count: Set(0),
            item_count: Set(0),
            byte_count: Set(0),
            digest: Set(None),
            status: Set(status.into()),
            incomplete_reason: Set(incomplete_reason.map(str::to_string)),
            created_at: Set(created_at),
            sealed_at: Set(None),
            capture_metadata: Set(None),
        }
        .insert(db)
        .await
        .expect("insert synthetic payload manifest");
    }

    fn normalize_manifest_page(body: &mut Value) {
        for manifest in body["items"].as_array_mut().expect("manifest items") {
            manifest["created_at"] = json!("<created_at>");
        }
    }

    fn expected_manifest(
        task_id: Uuid,
        id: Uuid,
        status: &str,
        incomplete_reason: Option<&str>,
        created_at: &str,
    ) -> Value {
        json!({
            "id": id,
            "task_id": task_id,
            "execution_step_id": null,
            "kind": "input",
            "source": "synthetic-source",
            "source_revision": "synthetic-revision",
            "source_locator": {"record": "synthetic"},
            "source_metadata": {"origin": "synthetic"},
            "schema": {"type": "array"},
            "capture_metadata": null,
            "root_is_array": true,
            "chunk_count": 0,
            "item_count": 0,
            "byte_count": 0,
            "digest": null,
            "status": status,
            "incomplete_reason": incomplete_reason,
            "created_at": created_at,
            "sealed_at": null,
        })
    }

    #[backend_test_macros::database_test]
    async fn payload_manifest_pages_are_task_scoped_and_stable_when_a_staging_manifest_is_added(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "synthetic-strategy").await;
        let task_id =
            insert_test_strategy_task(&db, strategy_id, "synthetic prompt", None, timestamp(0))
                .await;
        let other_task_id = insert_test_strategy_task(
            &db,
            strategy_id,
            "other synthetic prompt",
            None,
            timestamp(0),
        )
        .await;

        let first_id = Uuid::from_u128(0x810);
        let boundary_id = Uuid::from_u128(0x812);
        let later_id = Uuid::from_u128(0x813);
        let added_id = Uuid::from_u128(0x811);
        let other_task_manifest_id = Uuid::from_u128(0x814);
        insert_manifest(
            &db,
            task_id,
            first_id,
            timestamp(0),
            "incomplete",
            Some("synthetic-capture-incomplete"),
        )
        .await;
        insert_manifest(&db, task_id, boundary_id, timestamp(1), "staging", None).await;
        insert_manifest(
            &db,
            task_id,
            later_id,
            timestamp(2),
            "incomplete",
            Some("synthetic-provider-incomplete"),
        )
        .await;
        insert_manifest(
            &db,
            other_task_id,
            other_task_manifest_id,
            timestamp(1),
            "staging",
            None,
        )
        .await;

        let path =
            format!("/api/strategies/{strategy_id}/tasks/{task_id}/execution-artifacts/payloads");
        let first_response = server.get(&format!("{path}?limit=2")).await;
        let mut first_body: Value = first_response.json();
        normalize_manifest_page(&mut first_body);
        let next_cursor = first_body["next_cursor"]
            .as_str()
            .expect("next manifest cursor")
            .to_string();

        insert_manifest(&db, task_id, added_id, timestamp(1), "staging", None).await;

        let encoded_cursor = encode_query_cursor(&next_cursor);
        let second_response = server
            .get(&format!("{path}?cursor={encoded_cursor}&limit=2"))
            .await;
        let mut second_body: Value = second_response.json();
        normalize_manifest_page(&mut second_body);

        let refreshed_response = server.get(&format!("{path}?limit=2")).await;
        let mut refreshed_body: Value = refreshed_response.json();
        normalize_manifest_page(&mut refreshed_body);
        let oversized_response = server.get(&format!("{path}?limit=101")).await;
        let oversized_body: Value = oversized_response.json();

        assert_eq!(
            (
                first_response.status_code(),
                first_body,
                second_response.status_code(),
                second_body,
                refreshed_response.status_code(),
                refreshed_body,
                oversized_response.status_code(),
                oversized_body,
            ),
            (
                axum::http::StatusCode::OK,
                json!({
                    "task_id": task_id,
                    "cursor": null,
                    "next_cursor": next_cursor,
                    "total_count": 3,
                    "items": [
                        expected_manifest(
                            task_id,
                            first_id,
                            "incomplete",
                            Some("synthetic-capture-incomplete"),
                            "<created_at>",
                        ),
                        expected_manifest(
                            task_id,
                            boundary_id,
                            "staging",
                            None,
                            "<created_at>",
                        ),
                    ],
                }),
                axum::http::StatusCode::OK,
                json!({
                    "task_id": task_id,
                    "cursor": next_cursor,
                    "next_cursor": null,
                    "total_count": 4,
                    "items": [expected_manifest(
                        task_id,
                        later_id,
                        "incomplete",
                        Some("synthetic-provider-incomplete"),
                        "<created_at>",
                    )],
                }),
                axum::http::StatusCode::OK,
                json!({
                    "task_id": task_id,
                    "cursor": null,
                    "next_cursor": cursor(timestamp(1), added_id),
                    "total_count": 4,
                    "items": [
                        expected_manifest(
                            task_id,
                            first_id,
                            "incomplete",
                            Some("synthetic-capture-incomplete"),
                            "<created_at>",
                        ),
                        expected_manifest(
                            task_id,
                            added_id,
                            "staging",
                            None,
                            "<created_at>",
                        ),
                    ],
                }),
                axum::http::StatusCode::BAD_REQUEST,
                json!({"error": "invalid execution payload page request"}),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn payload_manifest_list_hides_a_task_from_another_strategy(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "synthetic-strategy").await;
        let other_strategy_id = insert_test_strategy(&db, "other-synthetic-strategy").await;
        let task_id =
            insert_test_strategy_task(&db, strategy_id, "synthetic prompt", None, timestamp(0))
                .await;
        insert_manifest(
            &db,
            task_id,
            Uuid::from_u128(0x820),
            timestamp(0),
            "incomplete",
            Some("synthetic-capture-incomplete"),
        )
        .await;

        let response = server
            .get(&format!(
                "/api/strategies/{other_strategy_id}/tasks/{task_id}/execution-artifacts/payloads"
            ))
            .await;
        let body: Value = response.json();

        assert_eq!(
            (response.status_code(), body),
            (
                axum::http::StatusCode::NOT_FOUND,
                json!({"error": "task execution artifact not found"}),
            ),
        );
    }
}
