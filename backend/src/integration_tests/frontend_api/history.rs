#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use gateway_postgres::entities::change_history;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use serde_json::json;
    use uuid::Uuid;

    use crate::testing::create_test_server;

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
        response.assert_status(StatusCode::OK);

        assert_eq!(
            response.json::<serde_json::Value>(),
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
        );
    }

    #[backend_test_macros::database_test]
    async fn get_history_returns_404_for_unknown_id(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let response = server
            .get(&format!("/api/history/{}", uuid::Uuid::from_u128(1)))
            .await;

        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    }
}
