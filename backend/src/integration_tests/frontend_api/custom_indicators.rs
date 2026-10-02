#[cfg(test)]
mod tests {
    use super::super::{assert_response_eq, create_strategy};
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

    use crate::testing::{create_test_server, create_test_server_with_db};
    use axum::http::StatusCode;
    use gateway_postgres::entities::change_history;
    use serde_json::{Value, json};
    use uuid::Uuid;

    fn create_payload(name: &str, code: &str) -> serde_json::Value {
        json!({
            "name": name,
            "code": code,
            "input_schema": {"type": "object"},
            "output_schema": {"type": "object"},
        })
    }

    fn normalize_dynamic(body: &mut serde_json::Value) {
        for key in ["indicator_id", "created_at", "updated_at"] {
            if body.get(key).is_some() {
                body[key] = json!("<dyn>");
            }
        }
    }

    fn expected_history_row(target_id: &str, op: &str, diff_json: Value) -> Value {
        json!({
            "id": "<dyn>",
            "target_kind": "custom_indicator",
            "target_id": target_id,
            "actor_kind": "human",
            "actor_label": "user",
            "op": op,
            "diff_json": diff_json,
            "summary": null,
            "created_at": "<dyn>",
        })
    }

    /// `/api/history` は created_at 降順で全件を返す。
    async fn latest_history_for(
        server: &axum_test::TestServer,
        indicator_id: &str,
        expected_rows: Vec<Value>,
    ) {
        let res = server
            .get(&format!(
                "/api/history?target_kind=custom_indicator&target_id={indicator_id}"
            ))
            .await;
        let mut rows = res.json::<Vec<Value>>();
        for row in &mut rows {
            for key in ["id", "created_at"] {
                row[key] = json!("<dyn>");
            }
        }
        assert_eq!((res.status_code(), rows), (StatusCode::OK, expected_rows));
    }

    async fn set_history_created_at_to_epoch(db: &impl sea_orm::ConnectionTrait, target_id: &str) {
        let row = change_history::Entity::find()
            .filter(
                change_history::Column::TargetId
                    .eq(Uuid::parse_str(target_id).expect("indicator UUID")),
            )
            .one(db)
            .await
            .expect("find history row")
            .expect("history row exists");
        change_history::ActiveModel {
            id: Set(row.id),
            created_at: Set(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset()),
            ..Default::default()
        }
        .update(db)
        .await
        .expect("set history row time");
    }

    #[backend_test_macros::database_test]
    async fn create_global_indicator_returns_201(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/indicators")
            .json(&create_payload("rsi", "print('{}')"))
            .await;
        let mut body = res.json::<serde_json::Value>();
        normalize_dynamic(&mut body);
        assert_eq!(
            (res.status_code(), body),
            (
                StatusCode::CREATED,
                json!({
                    "indicator_id": "<dyn>",
                    "name": "rsi",
                    "scope": "global",
                    "strategy_id": null,
                    "code": "print('{}')",
                    "input_schema": {"type": "object"},
                    "output_schema": {"type": "object"},
                    "description": null,
                    "created_at": "<dyn>",
                    "updated_at": "<dyn>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_strategy_indicator_returns_201(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let strategy_id = Uuid::parse_str(&create_strategy(&server, "s1").await).expect("uuid");

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/indicators"))
            .json(&create_payload("rsi", "print('{}')"))
            .await;
        let mut body = res.json::<serde_json::Value>();
        normalize_dynamic(&mut body);
        assert_eq!(
            (res.status_code(), body),
            (
                StatusCode::CREATED,
                json!({
                    "indicator_id": "<dyn>",
                    "name": "rsi",
                    "scope": "strategy",
                    "strategy_id": strategy_id.to_string(),
                    "code": "print('{}')",
                    "input_schema": {"type": "object"},
                    "output_schema": {"type": "object"},
                    "description": null,
                    "created_at": "<dyn>",
                    "updated_at": "<dyn>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_records_change_history(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/indicators")
            .json(&create_payload("rsi", "print('{}')"))
            .await;
        let id = res.json::<serde_json::Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");

        latest_history_for(
            &server,
            &id,
            vec![expected_history_row(
                &id,
                "create",
                json!({"name": "rsi", "scope": "global", "strategy_id": null}),
            )],
        )
        .await;
    }

    #[backend_test_macros::database_test]
    async fn empty_name_returns_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/indicators")
            .json(&create_payload("   ", "print('{}')"))
            .await;
        assert_response_eq(
            &res,
            StatusCode::BAD_REQUEST,
            Some(json!({ "error": "name must not be empty" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn input_schema_non_object_returns_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let mut payload = create_payload("rsi", "print('{}')");
        payload["input_schema"] = json!([1, 2, 3]);
        let res = server.post("/api/indicators").json(&payload).await;
        assert_response_eq(
            &res,
            StatusCode::BAD_REQUEST,
            Some(json!({ "error": "input_schema must be a JSON object" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn output_schema_non_object_returns_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let mut payload = create_payload("rsi", "print('{}')");
        payload["output_schema"] = json!("not-object");
        let res = server.post("/api/indicators").json(&payload).await;
        assert_response_eq(
            &res,
            StatusCode::BAD_REQUEST,
            Some(json!({ "error": "output_schema must be a JSON object" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn duplicate_global_name_returns_409(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let payload = create_payload("rsi", "print('{}')");
        server.post("/api/indicators").json(&payload).await;
        let second = server.post("/api/indicators").json(&payload).await;
        assert_response_eq(
            &second,
            StatusCode::CONFLICT,
            Some(json!({ "error": "resource already exists" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn duplicate_strategy_name_returns_409(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let strategy_id = Uuid::parse_str(&create_strategy(&server, "s1").await).expect("uuid");
        let payload = create_payload("rsi", "print('{}')");
        server
            .post(&format!("/api/strategies/{strategy_id}/indicators"))
            .json(&payload)
            .await;
        let second = server
            .post(&format!("/api/strategies/{strategy_id}/indicators"))
            .json(&payload)
            .await;
        assert_response_eq(
            &second,
            StatusCode::CONFLICT,
            Some(json!({ "error": "resource already exists" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn global_and_strategy_can_share_name(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let strategy_id = Uuid::parse_str(&create_strategy(&server, "s1").await).expect("uuid");
        let payload = create_payload("rsi", "print('{}')");

        let g = server.post("/api/indicators").json(&payload).await;
        let mut global_body = g.json::<serde_json::Value>();
        normalize_dynamic(&mut global_body);
        assert_eq!(
            (g.status_code(), global_body),
            (
                StatusCode::CREATED,
                json!({
                    "indicator_id": "<dyn>", "name": "rsi", "scope": "global",
                    "strategy_id": null, "code": "print('{}')",
                    "input_schema": {"type": "object"},
                    "output_schema": {"type": "object"}, "description": null,
                    "created_at": "<dyn>", "updated_at": "<dyn>",
                }),
            ),
        );

        let s = server
            .post(&format!("/api/strategies/{strategy_id}/indicators"))
            .json(&payload)
            .await;
        let mut strategy_body = s.json::<serde_json::Value>();
        normalize_dynamic(&mut strategy_body);
        assert_eq!(
            (s.status_code(), strategy_body),
            (
                StatusCode::CREATED,
                json!({
                    "indicator_id": "<dyn>", "name": "rsi", "scope": "strategy",
                    "strategy_id": strategy_id.to_string(), "code": "print('{}')",
                    "input_schema": {"type": "object"},
                    "output_schema": {"type": "object"}, "description": null,
                    "created_at": "<dyn>", "updated_at": "<dyn>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_isolates_strategy_scopes(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let s_a = Uuid::parse_str(&create_strategy(&server, "a").await).expect("uuid");
        let s_b = Uuid::parse_str(&create_strategy(&server, "b").await).expect("uuid");
        server
            .post(&format!("/api/strategies/{s_a}/indicators"))
            .json(&create_payload("only-a", "print('{}')"))
            .await;
        server
            .post(&format!("/api/strategies/{s_b}/indicators"))
            .json(&create_payload("only-b", "print('{}')"))
            .await;
        server
            .post("/api/indicators")
            .json(&create_payload("global", "print('{}')"))
            .await;

        let list_a = server
            .get(&format!("/api/strategies/{s_a}/indicators"))
            .await;
        let mut body_a: Vec<serde_json::Value> = list_a.json();
        body_a.iter_mut().for_each(normalize_dynamic);
        assert_eq!(
            (list_a.status_code(), body_a),
            (
                StatusCode::OK,
                vec![json!({
                    "indicator_id": "<dyn>", "name": "only-a", "scope": "strategy",
                    "strategy_id": s_a.to_string(), "code": "print('{}')",
                    "input_schema": {"type": "object"},
                    "output_schema": {"type": "object"}, "description": null,
                    "created_at": "<dyn>", "updated_at": "<dyn>",
                })],
            ),
        );

        let globals = server.get("/api/indicators").await;
        let mut body_g: Vec<serde_json::Value> = globals.json();
        body_g.iter_mut().for_each(normalize_dynamic);
        assert_eq!(
            (globals.status_code(), body_g),
            (
                StatusCode::OK,
                vec![json!({
                    "indicator_id": "<dyn>", "name": "global", "scope": "global",
                    "strategy_id": null, "code": "print('{}')",
                    "input_schema": {"type": "object"},
                    "output_schema": {"type": "object"}, "description": null,
                    "created_at": "<dyn>", "updated_at": "<dyn>",
                })],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_strategy_indicator_from_other_strategy_returns_404(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let s_a = Uuid::parse_str(&create_strategy(&server, "a").await).expect("uuid");
        let s_b = Uuid::parse_str(&create_strategy(&server, "b").await).expect("uuid");
        let created = server
            .post(&format!("/api/strategies/{s_a}/indicators"))
            .json(&create_payload("only-a", "print('{}')"))
            .await;
        let id = created.json::<serde_json::Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");

        let res = server
            .get(&format!("/api/strategies/{s_b}/indicators/{id}"))
            .await;
        assert_response_eq(
            &res,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": format!("indicator {id} not found") })),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_changes_fields(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let created = server
            .post("/api/indicators")
            .json(&create_payload("rsi", "old"))
            .await;
        let id = created.json::<serde_json::Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");

        let updated = server
            .put(&format!("/api/indicators/{id}"))
            .json(&json!({"code": "new", "description": "rsi indicator"}))
            .await;
        let mut body = updated.json::<serde_json::Value>();
        normalize_dynamic(&mut body);
        assert_eq!(
            (updated.status_code(), body),
            (
                StatusCode::OK,
                json!({
                    "indicator_id": "<dyn>",
                    "name": "rsi",
                    "scope": "global",
                    "strategy_id": null,
                    "code": "new",
                    "input_schema": {"type": "object"},
                    "output_schema": {"type": "object"},
                    "description": "rsi indicator",
                    "created_at": "<dyn>",
                    "updated_at": "<dyn>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_records_change_history_diff(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let created = server
            .post("/api/indicators")
            .json(&create_payload("rsi", "old"))
            .await;
        let id = created.json::<serde_json::Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");
        set_history_created_at_to_epoch(&db, &id).await;

        let updated = server
            .put(&format!("/api/indicators/{id}"))
            .json(&json!({"code": "new", "description": "rsi indicator"}))
            .await;
        let mut updated_body = updated.json::<serde_json::Value>();
        normalize_dynamic(&mut updated_body);
        assert_eq!(
            (updated.status_code(), updated_body),
            (
                StatusCode::OK,
                json!({
                    "indicator_id": "<dyn>", "name": "rsi", "scope": "global",
                    "strategy_id": null, "code": "new",
                    "input_schema": {"type": "object"},
                    "output_schema": {"type": "object"}, "description": "rsi indicator",
                    "created_at": "<dyn>", "updated_at": "<dyn>",
                }),
            ),
        );

        latest_history_for(
            &server,
            &id,
            vec![
                expected_history_row(
                    &id,
                    "update",
                    json!({
                        "code": {"len_from": 3, "len_to": 3},
                        "description": {"from": null, "to": "rsi indicator"},
                    }),
                ),
                expected_history_row(
                    &id,
                    "create",
                    json!({"name": "rsi", "scope": "global", "strategy_id": null}),
                ),
            ],
        )
        .await;
    }

    #[backend_test_macros::database_test]
    async fn delete_removes_indicator(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let created = server
            .post("/api/indicators")
            .json(&create_payload("rsi", "print('{}')"))
            .await;
        let id = created.json::<serde_json::Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");

        let del = server.delete(&format!("/api/indicators/{id}")).await;
        assert_response_eq(&del, StatusCode::NO_CONTENT, None);
        let get = server.get(&format!("/api/indicators/{id}")).await;
        assert_response_eq(
            &get,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": format!("indicator {id} not found") })),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_records_change_history(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let created = server
            .post("/api/indicators")
            .json(&create_payload("rsi", "print('{}')"))
            .await;
        let id = created.json::<serde_json::Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");
        set_history_created_at_to_epoch(&db, &id).await;

        let deleted = server.delete(&format!("/api/indicators/{id}")).await;
        assert_response_eq(&deleted, StatusCode::NO_CONTENT, None);

        latest_history_for(
            &server,
            &id,
            vec![
                expected_history_row(&id, "delete", json!({})),
                expected_history_row(
                    &id,
                    "create",
                    json!({"name": "rsi", "scope": "global", "strategy_id": null}),
                ),
            ],
        )
        .await;
    }

    mod preview {
        use super::*;
        use crate::testing::create_test_server_with_kata;
        use core_application::kata_exec::{ExecResult, FakeKataExecutor, SharedKataExecutor};
        use std::sync::Arc;

        fn preview_payload(code: &str) -> serde_json::Value {
            json!({
                "code": code,
                "input_schema": {"type": "object"},
                "output_schema": {"type": "object"},
                "args": {},
            })
        }

        #[backend_test_macros::database_test]
        async fn preview_returns_validated_output(db: gateway_postgres::DatabaseHandle) {
            let executor = Arc::new(FakeKataExecutor::new());
            executor
                .set_response(Ok(ExecResult {
                    stdout: "{\"value\": 42}\n".into(),
                    stderr: String::new(),
                    exit_code: 0,
                }))
                .await;
            let shared: SharedKataExecutor = executor.clone();
            let server = create_test_server_with_kata(db, shared).await;

            let res = server
                .post("/api/indicators/preview")
                .json(&json!({
                    "code": "print('{\"value\": 42}')",
                    "input_schema": {"type": "object", "properties": {"period": {"type": "integer"}}, "required": ["period"]},
                    "output_schema": {"type": "object", "properties": {"value": {"type": "number"}}, "required": ["value"]},
                    "args": {"period": 14},
                }))
                .await;
            assert_response_eq(
                &res,
                StatusCode::OK,
                Some(json!({
                    "output": {"value": 42},
                    "stdout": "{\"value\": 42}\n",
                    "stderr": "",
                    "exit_code": 0,
                })),
            );

            let recorded = executor.requests.lock().await;
            assert_eq!(
                recorded.as_slice(),
                &[core_application::kata_exec::ExecRequest {
                    code: "print('{\"value\": 42}')".into(),
                    stdin: Some(r#"{"args":{"period":14}}"#.into()),
                    timeout: None,
                    max_output_bytes: None,
                }],
            );
        }

        #[backend_test_macros::database_test]
        async fn preview_returns_400_for_input_schema_mismatch(
            db: gateway_postgres::DatabaseHandle,
        ) {
            let executor = Arc::new(FakeKataExecutor::new());
            let shared: SharedKataExecutor = executor.clone();
            let server = create_test_server_with_kata(db, shared).await;

            let res = server
                .post("/api/indicators/preview")
                .json(&json!({
                    "code": "print('{}')",
                    "input_schema": {
                        "type": "object",
                        "properties": {"period": {"type": "integer"}},
                        "required": ["period"],
                    },
                    "output_schema": {"type": "object"},
                    "args": {"period": "not-int"},
                }))
                .await;
            assert_response_eq(
                &res,
                StatusCode::BAD_REQUEST,
                Some(json!({
                    "error": "value does not match input_schema: \"not-int\" is not of type \"integer\" at /period"
                })),
            );
            assert!(executor.requests.lock().await.is_empty());
        }

        #[backend_test_macros::database_test]
        async fn preview_passes_through_sandbox_rejection(db: gateway_postgres::DatabaseHandle) {
            let executor = Arc::new(FakeKataExecutor::new());
            executor
                .set_response(Ok(ExecResult {
                    stdout: String::new(),
                    stderr: "PermissionError: network access denied".into(),
                    exit_code: 1,
                }))
                .await;
            let shared: SharedKataExecutor = executor;
            let server = create_test_server_with_kata(db, shared).await;

            let res = server
                .post("/api/indicators/preview")
                .json(&preview_payload(
                    "import urllib.request; urllib.request.urlopen('http://x')",
                ))
                .await;
            assert_response_eq(
                &res,
                StatusCode::OK,
                Some(json!({
                    "output": null,
                    "stdout": "",
                    "stderr": "PermissionError: network access denied",
                    "exit_code": 1,
                })),
            );
        }

        #[backend_test_macros::database_test]
        async fn preview_returns_503_when_executor_disabled(db: gateway_postgres::DatabaseHandle) {
            let server = create_test_server(db).await;
            let res = server
                .post("/api/indicators/preview")
                .json(&preview_payload("print('{}')"))
                .await;
            assert_response_eq(
                &res,
                StatusCode::SERVICE_UNAVAILABLE,
                Some(json!({ "error": "indicator runtime is not configured" })),
            );
        }

        #[backend_test_macros::database_test]
        async fn preview_returns_200_with_validation_error_in_stderr_for_invalid_output(
            db: gateway_postgres::DatabaseHandle,
        ) {
            let executor = Arc::new(FakeKataExecutor::new());
            executor
                .set_response(Ok(ExecResult {
                    stdout: "not-json\n".into(),
                    stderr: String::new(),
                    exit_code: 0,
                }))
                .await;
            let shared: SharedKataExecutor = executor;
            let server = create_test_server_with_kata(db, shared).await;

            let res = server
                .post("/api/indicators/preview")
                .json(&preview_payload("print('not-json')"))
                .await;
            let mut body = res.json::<serde_json::Value>();
            // stderr の本文は jsonschema の詳細メッセージに依存して変動するため、
            // prefix が出ているかだけを spec として固定する。
            let stderr_starts_correctly = body["stderr"]
                .as_str()
                .is_some_and(|s| s.starts_with("Output validation error: "));
            body["stderr"] = json!(stderr_starts_correctly);
            assert_eq!(
                (res.status_code(), body),
                (
                    StatusCode::OK,
                    json!({
                        "output": null,
                        "stdout": "not-json\n",
                        "stderr": true,
                        "exit_code": 0,
                    }),
                ),
            );
        }
    }
}
