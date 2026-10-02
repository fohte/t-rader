#[cfg(test)]
mod tests {
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

    use crate::testing::{create_test_server, create_test_server_with_db};
    use axum::http::StatusCode;
    use gateway_postgres::entities::change_history;
    use serde_json::json;
    use uuid::Uuid;
    async fn create_strategy(server: &axum_test::TestServer, name: &str) -> Uuid {
        let res = server
            .post("/api/strategies")
            .json(&json!({ "name": name }))
            .await;
        res.assert_status(StatusCode::CREATED);
        let id = res.json::<serde_json::Value>()["id"]
            .as_str()
            .map(str::to_string)
            .expect("id");
        Uuid::parse_str(&id).expect("uuid")
    }

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

    /// 指定 indicator の最新の change_history 行を返す (`/api/history` は created_at 降順)
    async fn latest_history_for(
        server: &axum_test::TestServer,
        indicator_id: &str,
    ) -> serde_json::Value {
        let res = server
            .get(&format!(
                "/api/history?target_kind=custom_indicator&target_id={indicator_id}"
            ))
            .await;
        res.assert_status_ok();
        let mut rows = res.json::<Vec<serde_json::Value>>();
        assert!(!rows.is_empty(), "expected at least one history row");
        let mut row = rows.remove(0);
        for key in ["id", "created_at"] {
            row[key] = json!("<dyn>");
        }
        row
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
        res.assert_status(StatusCode::CREATED);
        let mut body = res.json::<serde_json::Value>();
        normalize_dynamic(&mut body);
        assert_eq!(
            body,
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
        );
    }

    #[backend_test_macros::database_test]
    async fn create_strategy_indicator_returns_201(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let strategy_id = create_strategy(&server, "s1").await;

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/indicators"))
            .json(&create_payload("rsi", "print('{}')"))
            .await;
        res.assert_status(StatusCode::CREATED);
        let mut body = res.json::<serde_json::Value>();
        normalize_dynamic(&mut body);
        assert_eq!(
            body,
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

        assert_eq!(
            latest_history_for(&server, &id).await,
            json!({
                "id": "<dyn>",
                "target_kind": "custom_indicator",
                "target_id": id,
                "actor_kind": "human",
                "actor_label": "user",
                "op": "create",
                "diff_json": {"name": "rsi", "scope": "global", "strategy_id": null},
                "summary": null,
                "created_at": "<dyn>",
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn empty_name_returns_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/indicators")
            .json(&create_payload("   ", "print('{}')"))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[backend_test_macros::database_test]
    async fn input_schema_non_object_returns_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let mut payload = create_payload("rsi", "print('{}')");
        payload["input_schema"] = json!([1, 2, 3]);
        let res = server.post("/api/indicators").json(&payload).await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[backend_test_macros::database_test]
    async fn output_schema_non_object_returns_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let mut payload = create_payload("rsi", "print('{}')");
        payload["output_schema"] = json!("not-object");
        let res = server.post("/api/indicators").json(&payload).await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[backend_test_macros::database_test]
    async fn duplicate_global_name_returns_409(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let payload = create_payload("rsi", "print('{}')");
        server.post("/api/indicators").json(&payload).await;
        let second = server.post("/api/indicators").json(&payload).await;
        second.assert_status(StatusCode::CONFLICT);
    }

    #[backend_test_macros::database_test]
    async fn duplicate_strategy_name_returns_409(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let strategy_id = create_strategy(&server, "s1").await;
        let payload = create_payload("rsi", "print('{}')");
        server
            .post(&format!("/api/strategies/{strategy_id}/indicators"))
            .json(&payload)
            .await;
        let second = server
            .post(&format!("/api/strategies/{strategy_id}/indicators"))
            .json(&payload)
            .await;
        second.assert_status(StatusCode::CONFLICT);
    }

    #[backend_test_macros::database_test]
    async fn global_and_strategy_can_share_name(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let strategy_id = create_strategy(&server, "s1").await;
        let payload = create_payload("rsi", "print('{}')");

        let g = server.post("/api/indicators").json(&payload).await;
        g.assert_status(StatusCode::CREATED);

        let s = server
            .post(&format!("/api/strategies/{strategy_id}/indicators"))
            .json(&payload)
            .await;
        s.assert_status(StatusCode::CREATED);
    }

    #[backend_test_macros::database_test]
    async fn list_isolates_strategy_scopes(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let s_a = create_strategy(&server, "a").await;
        let s_b = create_strategy(&server, "b").await;
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
        list_a.assert_status_ok();
        let body_a: Vec<serde_json::Value> = list_a.json();
        let names_a: Vec<&str> = body_a.iter().map(|v| v["name"].as_str().unwrap()).collect();
        assert_eq!(names_a, vec!["only-a"]);

        let globals = server.get("/api/indicators").await;
        globals.assert_status_ok();
        let body_g: Vec<serde_json::Value> = globals.json();
        let names_g: Vec<&str> = body_g.iter().map(|v| v["name"].as_str().unwrap()).collect();
        assert_eq!(names_g, vec!["global"]);
    }

    #[backend_test_macros::database_test]
    async fn get_strategy_indicator_from_other_strategy_returns_404(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let s_a = create_strategy(&server, "a").await;
        let s_b = create_strategy(&server, "b").await;
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
        res.assert_status(StatusCode::NOT_FOUND);
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
        updated.assert_status_ok();
        let mut body = updated.json::<serde_json::Value>();
        normalize_dynamic(&mut body);
        assert_eq!(
            body,
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

        server
            .put(&format!("/api/indicators/{id}"))
            .json(&json!({"code": "new", "description": "rsi indicator"}))
            .await
            .assert_status_ok();

        assert_eq!(
            latest_history_for(&server, &id).await,
            json!({
                "id": "<dyn>",
                "target_kind": "custom_indicator",
                "target_id": id,
                "actor_kind": "human",
                "actor_label": "user",
                "op": "update",
                "diff_json": {
                    "code": {"len_from": 3, "len_to": 3},
                    "description": {"from": null, "to": "rsi indicator"},
                },
                "summary": null,
                "created_at": "<dyn>",
            }),
        );
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
        del.assert_status(StatusCode::NO_CONTENT);
        let get = server.get(&format!("/api/indicators/{id}")).await;
        get.assert_status(StatusCode::NOT_FOUND);
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

        server
            .delete(&format!("/api/indicators/{id}"))
            .await
            .assert_status(StatusCode::NO_CONTENT);

        assert_eq!(
            latest_history_for(&server, &id).await,
            json!({
                "id": "<dyn>",
                "target_kind": "custom_indicator",
                "target_id": id,
                "actor_kind": "human",
                "actor_label": "user",
                "op": "delete",
                "diff_json": {},
                "summary": null,
                "created_at": "<dyn>",
            }),
        );
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
            res.assert_status_ok();
            assert_eq!(
                res.json::<serde_json::Value>(),
                json!({
                    "output": {"value": 42},
                    "stdout": "{\"value\": 42}\n",
                    "stderr": "",
                    "exit_code": 0,
                }),
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
            res.assert_status(StatusCode::BAD_REQUEST);
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
            res.assert_status_ok();
            assert_eq!(
                res.json::<serde_json::Value>(),
                json!({
                    "output": null,
                    "stdout": "",
                    "stderr": "PermissionError: network access denied",
                    "exit_code": 1,
                }),
            );
        }

        #[backend_test_macros::database_test]
        async fn preview_returns_503_when_executor_disabled(db: gateway_postgres::DatabaseHandle) {
            let server = create_test_server(db).await;
            let res = server
                .post("/api/indicators/preview")
                .json(&preview_payload("print('{}')"))
                .await;
            res.assert_status(StatusCode::SERVICE_UNAVAILABLE);
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
            res.assert_status_ok();
            let mut body = res.json::<serde_json::Value>();
            // stderr の本文は jsonschema の詳細メッセージに依存して変動するため、
            // prefix が出ているかだけを spec として固定する。
            let stderr_starts_correctly = body["stderr"]
                .as_str()
                .is_some_and(|s| s.starts_with("Output validation error: "));
            body["stderr"] = json!(stderr_starts_correctly);
            assert_eq!(
                body,
                json!({
                    "output": null,
                    "stdout": "not-json\n",
                    "stderr": true,
                    "exit_code": 0,
                }),
            );
        }
    }
}
