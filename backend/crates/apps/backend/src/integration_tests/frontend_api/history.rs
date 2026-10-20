#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
    use axum::http::StatusCode;
    use chrono::{DateTime, FixedOffset, Utc};
    use gateway_postgres::entities::change_history;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::testing::create_test_server;

    fn history_timestamp() -> DateTime<FixedOffset> {
        DateTime::<Utc>::UNIX_EPOCH.fixed_offset()
    }

    fn history_model(id: Uuid, target_kind: &str, target_id: Uuid) -> change_history::ActiveModel {
        change_history::ActiveModel {
            id: Set(id),
            target_kind: Set(target_kind.to_string()),
            target_id: Set(target_id),
            actor_kind: Set("human".to_string()),
            actor_label: Set("test-operator".to_string()),
            op: Set("update".to_string()),
            diff_json: Set(json!({ "field": "test-value" })),
            summary: Set(None),
            created_at: Set(history_timestamp()),
        }
    }

    async fn insert_history(
        db: &gateway_postgres::DatabaseHandle,
        id: Uuid,
        target_kind: &str,
        target_id: Uuid,
    ) {
        history_model(id, target_kind, target_id)
            .insert(db)
            .await
            .expect("insert test history");
    }

    fn history_response(id: Uuid, target_id: Uuid) -> Value {
        json!({
            "id": id,
            "target_kind": "trigger",
            "target_id": target_id,
            "actor_kind": "human",
            "actor_label": "test-operator",
            "op": "update",
            "diff_json": { "field": "test-value" },
            "summary": null,
            "created_at": "1970-01-01T00:00:00Z",
        })
    }

    #[backend_test_macros::database_test]
    async fn get_history_returns_full_row(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db.clone()).await;
        let history_id = Uuid::from_u128(1);
        let target_id = Uuid::from_u128(2);
        change_history::ActiveModel {
            id: Set(history_id),
            target_kind: Set("note".to_string()),
            target_id: Set(target_id),
            actor_kind: Set("human".to_string()),
            actor_label: Set("sample_user".to_string()),
            op: Set("create".to_string()),
            diff_json: Set(json!({ "name": "sample_entry" })),
            summary: Set(Some("sample summary".to_string())),
            created_at: Set(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset()),
        }
        .insert(&db)
        .await
        .expect("insert change history");

        let response = server.get(&format!("/api/history/{history_id}")).await;
        assert_eq!(
            (response.status_code(), response.json::<serde_json::Value>()),
            (
                StatusCode::OK,
                json!({
                    "id": history_id,
                    "target_kind": "note",
                    "target_id": target_id,
                    "actor_kind": "human",
                    "actor_label": "sample_user",
                    "op": "create",
                    "diff_json": { "name": "sample_entry" },
                    "summary": "sample summary",
                    "created_at": "1970-01-01T00:00:00Z",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_history_returns_404_for_unknown_id(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let history_id = uuid::Uuid::from_u128(1);
        let response = server.get(&format!("/api/history/{history_id}")).await;
        assert_response_eq(
            &response,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": format!("history {history_id} not found") })),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_history_pages_stably_with_target_filters(db: gateway_postgres::DatabaseHandle) {
        const TARGET_ID_START: u128 = 81_000;
        const TARGET_HISTORY_COUNT: u128 = 502;
        let server = create_test_server(db.clone()).await;
        let target_id = Uuid::from_u128(80_000);
        for offset in 0..TARGET_HISTORY_COUNT {
            insert_history(
                &db,
                Uuid::from_u128(TARGET_ID_START + offset),
                "trigger",
                target_id,
            )
            .await;
        }
        insert_history(
            &db,
            Uuid::from_u128(80_999),
            "trigger",
            Uuid::from_u128(80_001),
        )
        .await;
        insert_history(&db, Uuid::from_u128(80_998), "agent_config", target_id).await;

        let first_page = server
            .get(&format!(
                "/api/history?target_kind=trigger&target_id={target_id}&limit=500"
            ))
            .await;
        let first_body = first_page.json::<Value>();
        let cursor_id = first_body
            .as_array()
            .and_then(|rows| rows.last())
            .and_then(|row| row.get("id"))
            .and_then(Value::as_str)
            .expect("first page has a final history id")
            .parse::<Uuid>()
            .expect("history id is a UUID");
        let second_page = server
            .get(&format!(
                "/api/history?target_kind=trigger&target_id={target_id}&limit=500&before={cursor_id}"
            ))
            .await;
        let second_body = second_page.json::<Value>();

        let expected: Vec<_> = (0..TARGET_HISTORY_COUNT)
            .rev()
            .map(|offset| history_response(Uuid::from_u128(TARGET_ID_START + offset), target_id))
            .collect();
        assert_eq!(
            (
                first_page.status_code(),
                first_body,
                second_page.status_code(),
                second_body,
            ),
            (
                StatusCode::OK,
                json!(expected[..500]),
                StatusCode::OK,
                json!(expected[500..]),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_history_rejects_missing_or_mismatched_cursors(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db.clone()).await;
        let target_id = Uuid::from_u128(91_000);
        let other_target_id = Uuid::from_u128(91_001);
        let other_kind_cursor = Uuid::from_u128(91_002);
        let other_target_cursor = Uuid::from_u128(91_003);
        let missing_cursor = Uuid::from_u128(91_004);
        insert_history(&db, other_kind_cursor, "agent_config", target_id).await;
        insert_history(&db, other_target_cursor, "trigger", target_id).await;

        let missing = server
            .get(&format!("/api/history?before={missing_cursor}"))
            .await;
        let kind_mismatch = server
            .get(&format!(
                "/api/history?target_kind=trigger&before={other_kind_cursor}"
            ))
            .await;
        let target_mismatch = server
            .get(&format!(
                "/api/history?target_kind=trigger&target_id={other_target_id}&before={other_target_cursor}"
            ))
            .await;

        assert_eq!(
            [
                (missing.status_code(), missing.json::<Value>()),
                (kind_mismatch.status_code(), kind_mismatch.json::<Value>()),
                (
                    target_mismatch.status_code(),
                    target_mismatch.json::<Value>()
                ),
            ],
            [
                (
                    StatusCode::BAD_REQUEST,
                    json!({ "error": "history cursor is invalid" }),
                ),
                (
                    StatusCode::BAD_REQUEST,
                    json!({ "error": "history cursor is invalid" }),
                ),
                (
                    StatusCode::BAD_REQUEST,
                    json!({ "error": "history cursor is invalid" }),
                ),
            ],
        );
    }
}
