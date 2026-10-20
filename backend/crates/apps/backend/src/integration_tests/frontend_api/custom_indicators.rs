#[cfg(test)]
mod tests {
    use super::super::{assert_response_eq, create_strategy};
    use sea_orm::{
        ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
        SqlxPostgresConnector,
    };

    use crate::testing::{create_test_server, create_test_server_with_db};
    use axum::http::StatusCode;
    use gateway_postgres::entities::change_history;
    use gateway_postgres::test_support::create_test_pool_with_max_connections;
    use serde_json::{Value, json};
    use uuid::Uuid;

    const INITIAL_RESTORABLE_CODE: &str = indoc::indoc! {"
        alpha
        beta!
    "};
    const UPDATED_RESTORABLE_CODE: &str = indoc::indoc! {"
        gamma
        delta
    "};
    const INITIAL_MULTILINE_CODE: &str = indoc::indoc! {"
        first
        line
    "};
    const UPDATED_MULTILINE_CODE: &str = indoc::indoc! {"
        other
        text
    "};

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

    fn expected_history_row(target_id: &str, op: &str, mut diff_json: Value) -> Value {
        for side in ["before", "after"] {
            if let Some(definition) = diff_json
                .as_object_mut()
                .and_then(|diff| diff.get_mut(side))
                .filter(|definition| definition.is_object())
            {
                definition["indicator_id"] = json!(target_id);
            }
        }
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

    fn expected_definition(
        name: &str,
        code: &str,
        input_schema: Value,
        output_schema: Value,
        description: Value,
    ) -> Value {
        json!({
            "indicator_id": "<dyn>",
            "name": name,
            "scope": "global",
            "strategy_id": null,
            "code": code,
            "input_schema": input_schema,
            "output_schema": output_schema,
            "description": description,
        })
    }

    fn expected_snapshot(before: Option<Value>, after: Option<Value>) -> Value {
        json!({
            "format": "definition_snapshot",
            "version": 1,
            "before": before,
            "after": after,
        })
    }

    fn expected_response(mut definition: Value) -> Value {
        definition["created_at"] = json!("<dyn>");
        definition["updated_at"] = json!("<dyn>");
        definition
    }

    fn normalize_history_rows(rows: &mut [Value]) {
        for row in rows {
            row["id"] = json!("<dyn>");
            row["created_at"] = json!("<dyn>");
            let target_id = row["target_id"].clone();
            for side in ["before", "after"] {
                if let Some(definition) = row
                    .get_mut("diff_json")
                    .and_then(Value::as_object_mut)
                    .and_then(|diff| diff.get_mut(side))
                    .filter(|definition| definition.is_object())
                {
                    definition["indicator_id"] = target_id.clone();
                }
            }
        }
    }

    async fn fetch_history_for(
        server: &axum_test::TestServer,
        indicator_id: &str,
    ) -> (StatusCode, Vec<Value>) {
        let res = server
            .get(&format!(
                "/api/history?target_kind=custom_indicator&target_id={indicator_id}"
            ))
            .await;
        let status = res.status_code();
        let mut rows = res.json::<Vec<Value>>();
        normalize_history_rows(&mut rows);
        (status, rows)
    }

    /// `/api/history` は created_at 降順で全件を返す。
    async fn latest_history_for(
        server: &axum_test::TestServer,
        indicator_id: &str,
        expected_rows: Vec<Value>,
    ) {
        let (status, rows) = fetch_history_for(server, indicator_id).await;
        assert_eq!((status, rows), (StatusCode::OK, expected_rows));
    }

    async fn set_history_created_at_to_epoch(db: &impl sea_orm::ConnectionTrait, target_id: &str) {
        let rows = change_history::Entity::find()
            .filter(
                change_history::Column::TargetId
                    .eq(Uuid::parse_str(target_id).expect("indicator UUID")),
            )
            .all(db)
            .await
            .expect("find history row");
        for row in rows {
            change_history::ActiveModel {
                id: Set(row.id),
                created_at: Set(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset()),
                ..Default::default()
            }
            .update(db)
            .await
            .expect("set history row time");
        }
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
    async fn create_records_full_definition_snapshot(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/indicators")
            .json(&create_payload("sample-indicator", "sample-code\n"))
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
                expected_snapshot(
                    None,
                    Some(expected_definition(
                        "sample-indicator",
                        "sample-code\n",
                        json!({"type": "object"}),
                        json!({"type": "object"}),
                        json!(null),
                    )),
                ),
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
    async fn history_restores_the_complete_previous_definition_through_put(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let initial_input_schema = json!({
            "type": "object",
            "properties": {"first": {"type": "string"}},
        });
        let initial_output_schema = json!({
            "type": "object",
            "properties": {"result": {"type": "string"}},
        });
        let mut initial_payload = create_payload("sample-indicator", INITIAL_RESTORABLE_CODE);
        initial_payload["input_schema"] = initial_input_schema.clone();
        initial_payload["output_schema"] = initial_output_schema.clone();
        let created = server.post("/api/indicators").json(&initial_payload).await;
        let id = created.json::<serde_json::Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");
        set_history_created_at_to_epoch(&db, &id).await;

        let updated_input_schema = json!({
            "type": "object",
            "properties": {"second": {"type": "string"}},
        });
        let updated_output_schema = json!({
            "type": "object",
            "properties": {"value": {"type": "integer"}},
        });
        let update_payload = json!({
            "name": "renamed-indicator",
            "code": UPDATED_RESTORABLE_CODE,
            "input_schema": updated_input_schema,
            "output_schema": updated_output_schema,
            "description": "",
        });
        let updated = server
            .put(&format!("/api/indicators/{id}"))
            .json(&update_payload)
            .await;
        let mut updated_body = updated.json::<serde_json::Value>();
        normalize_dynamic(&mut updated_body);
        let (first_history_status, first_history) = fetch_history_for(&server, &id).await;
        let before = first_history[0]["diff_json"]["before"].clone();
        let restore_payload = json!({
            "name": before["name"],
            "code": before["code"],
            "input_schema": before["input_schema"],
            "output_schema": before["output_schema"],
            "description": before["description"],
        });
        let restored = server
            .put(&format!("/api/indicators/{id}"))
            .json(&restore_payload)
            .await;
        let mut restored_body = restored.json::<Value>();
        normalize_dynamic(&mut restored_body);
        let (final_history_status, final_history) = fetch_history_for(&server, &id).await;

        assert_eq!(
            (
                updated.status_code(),
                updated_body,
                first_history_status,
                restored.status_code(),
                restored_body,
                final_history_status,
                final_history,
            ),
            (
                StatusCode::OK,
                {
                    let mut expected = expected_definition(
                        "renamed-indicator",
                        UPDATED_RESTORABLE_CODE,
                        json!({
                            "type": "object",
                            "properties": {"second": {"type": "string"}},
                        }),
                        json!({
                            "type": "object",
                            "properties": {"value": {"type": "integer"}},
                        }),
                        json!(""),
                    );
                    expected["created_at"] = json!("<dyn>");
                    expected["updated_at"] = json!("<dyn>");
                    expected
                },
                StatusCode::OK,
                StatusCode::OK,
                {
                    let mut expected = expected_definition(
                        "sample-indicator",
                        INITIAL_RESTORABLE_CODE,
                        initial_input_schema.clone(),
                        initial_output_schema.clone(),
                        json!(null),
                    );
                    expected["created_at"] = json!("<dyn>");
                    expected["updated_at"] = json!("<dyn>");
                    expected
                },
                StatusCode::OK,
                vec![
                    expected_history_row(
                        &id,
                        "update",
                        expected_snapshot(
                            Some(expected_definition(
                                "renamed-indicator",
                                UPDATED_RESTORABLE_CODE,
                                json!({
                                    "type": "object",
                                    "properties": {"second": {"type": "string"}},
                                }),
                                json!({
                                    "type": "object",
                                    "properties": {"value": {"type": "integer"}},
                                }),
                                json!(""),
                            )),
                            Some(expected_definition(
                                "sample-indicator",
                                INITIAL_RESTORABLE_CODE,
                                initial_input_schema.clone(),
                                initial_output_schema.clone(),
                                json!(null),
                            )),
                        ),
                    ),
                    expected_history_row(
                        &id,
                        "update",
                        expected_snapshot(
                            Some(expected_definition(
                                "sample-indicator",
                                INITIAL_RESTORABLE_CODE,
                                initial_input_schema.clone(),
                                initial_output_schema.clone(),
                                json!(null),
                            )),
                            Some(expected_definition(
                                "renamed-indicator",
                                UPDATED_RESTORABLE_CODE,
                                json!({
                                    "type": "object",
                                    "properties": {"second": {"type": "string"}},
                                }),
                                json!({
                                    "type": "object",
                                    "properties": {"value": {"type": "integer"}},
                                }),
                                json!(""),
                            )),
                        ),
                    ),
                    expected_history_row(
                        &id,
                        "create",
                        expected_snapshot(
                            None,
                            Some(expected_definition(
                                "sample-indicator",
                                INITIAL_RESTORABLE_CODE,
                                initial_input_schema,
                                initial_output_schema,
                                json!(null),
                            )),
                        ),
                    ),
                ],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_removes_indicator(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let created = server
            .post("/api/indicators")
            .json(&create_payload("sample-indicator", "sample-code\n"))
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
            .json(&create_payload("sample-indicator", "sample-code\n"))
            .await;
        let id = created.json::<serde_json::Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");
        set_history_created_at_to_epoch(&db, &id).await;

        let deleted = server.delete(&format!("/api/indicators/{id}")).await;
        let (history_status, history) = fetch_history_for(&server, &id).await;
        assert_eq!(
            (deleted.status_code(), history_status, history),
            (
                StatusCode::NO_CONTENT,
                StatusCode::OK,
                vec![
                    expected_history_row(
                        &id,
                        "delete",
                        expected_snapshot(
                            Some(expected_definition(
                                "sample-indicator",
                                "sample-code\n",
                                json!({"type": "object"}),
                                json!({"type": "object"}),
                                json!(null),
                            )),
                            None,
                        ),
                    ),
                    expected_history_row(
                        &id,
                        "create",
                        expected_snapshot(
                            None,
                            Some(expected_definition(
                                "sample-indicator",
                                "sample-code\n",
                                json!({"type": "object"}),
                                json!({"type": "object"}),
                                json!(null),
                            )),
                        ),
                    ),
                ],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn history_failure_rolls_back_the_indicator_update(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let initial_code = INITIAL_MULTILINE_CODE;
        let created = server
            .post("/api/indicators")
            .json(&create_payload("sample-indicator", initial_code))
            .await;
        let id = created.json::<Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");

        db.execute_unprepared(
            r#"CREATE FUNCTION reject_custom_indicator_history() RETURNS trigger
                LANGUAGE plpgsql AS $$
                BEGIN
                    IF NEW.target_kind = 'custom_indicator' THEN
                        RAISE EXCEPTION 'synthetic history failure';
                    END IF;
                    RETURN NEW;
                END;
                $$"#,
        )
        .await
        .expect("create history failure function");
        db.execute_unprepared(
            "CREATE TRIGGER reject_custom_indicator_history BEFORE INSERT ON change_history \
             FOR EACH ROW EXECUTE FUNCTION reject_custom_indicator_history()",
        )
        .await
        .expect("create history failure trigger");

        let failed_update = server
            .put(&format!("/api/indicators/{id}"))
            .json(&json!({"code": UPDATED_MULTILINE_CODE}))
            .await;
        let error_body = failed_update.json::<Value>();
        let current = server.get(&format!("/api/indicators/{id}")).await;
        let mut current_body = current.json::<Value>();
        normalize_dynamic(&mut current_body);
        let (history_status, history) = fetch_history_for(&server, &id).await;

        assert_eq!(
            (
                failed_update.status_code(),
                error_body,
                current.status_code(),
                current_body,
                history_status,
                history,
            ),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"error": "internal server error"}),
                StatusCode::OK,
                expected_response(expected_definition(
                    "sample-indicator",
                    initial_code,
                    json!({"type": "object"}),
                    json!({"type": "object"}),
                    json!(null),
                )),
                StatusCode::OK,
                vec![expected_history_row(
                    &id,
                    "create",
                    expected_snapshot(
                        None,
                        Some(expected_definition(
                            "sample-indicator",
                            initial_code,
                            json!({"type": "object"}),
                            json!({"type": "object"}),
                            json!(null),
                        )),
                    ),
                )],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn legacy_custom_indicator_history_remains_readable(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let target_id = Uuid::new_v4();
        let target_id_string = target_id.to_string();
        let legacy_diff = json!({"code": {"len_from": 4, "len_to": 4}});
        change_history::ActiveModel {
            id: Set(Uuid::new_v4()),
            target_kind: Set("custom_indicator".to_string()),
            target_id: Set(target_id),
            actor_kind: Set("human".to_string()),
            actor_label: Set("user".to_string()),
            op: Set("update".to_string()),
            diff_json: Set(legacy_diff.clone()),
            summary: Set(None),
            created_at: Set(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset()),
        }
        .insert(&db)
        .await
        .expect("insert legacy history row");
        let server = create_test_server(db).await;

        let (status, history) = fetch_history_for(&server, &target_id_string).await;

        assert_eq!(
            (status, history),
            (
                StatusCode::OK,
                vec![expected_history_row(
                    &target_id_string,
                    "update",
                    legacy_diff,
                )],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn concurrent_updates_capture_the_actual_previous_definition(
        test_transaction: gateway_postgres::DatabaseHandle,
    ) {
        let _test_transaction = test_transaction;
        let pool = create_test_pool_with_max_connections(4).await;
        let db = gateway_postgres::DatabaseHandle::from(
            SqlxPostgresConnector::from_sqlx_postgres_pool(pool),
        );
        let server = create_test_server(db.clone()).await;
        let initial_name = format!("sample-indicator-{}", Uuid::new_v4());
        let renamed = "renamed-indicator";
        let initial_code = INITIAL_MULTILINE_CODE;
        let changed_code = UPDATED_MULTILINE_CODE;
        let created = server
            .post("/api/indicators")
            .json(&create_payload(&initial_name, initial_code))
            .await;
        let id = created.json::<Value>()["indicator_id"]
            .as_str()
            .map(str::to_string)
            .expect("id");
        let indicator_id = Uuid::parse_str(&id).expect("indicator UUID");
        set_history_created_at_to_epoch(&db, &id).await;

        let (name_update, code_update) = tokio::join!(
            server
                .put(&format!("/api/indicators/{id}"))
                .json(&json!({"name": renamed})),
            server
                .put(&format!("/api/indicators/{id}"))
                .json(&json!({"code": changed_code})),
        );
        let name_status = name_update.status_code();
        let code_status = code_update.status_code();
        let mut name_body = name_update.json::<Value>();
        let mut code_body = code_update.json::<Value>();
        normalize_dynamic(&mut name_body);
        normalize_dynamic(&mut code_body);
        let current = server.get(&format!("/api/indicators/{id}")).await;
        let current_status = current.status_code();
        let mut current_body = current.json::<Value>();
        normalize_dynamic(&mut current_body);
        let (history_status, mut history) = fetch_history_for(&server, &id).await;
        let history_count = history.len();
        let mut update_history: Vec<_> = history
            .iter()
            .filter(|row| row["op"] == "update")
            .cloned()
            .collect();
        update_history.sort_by_key(|row| {
            let before = &row["diff_json"]["before"];
            !(before["name"].as_str() == Some(initial_name.as_str())
                && before["code"].as_str() == Some(initial_code))
        });
        let code_was_first = update_history
            .first()
            .is_some_and(|row| row["diff_json"]["after"]["code"].as_str() == Some(changed_code));
        let create_history: Vec<_> = history
            .drain(..)
            .filter(|row| row["op"] == "create")
            .collect();

        change_history::Entity::delete_many()
            .filter(change_history::Column::TargetId.eq(indicator_id))
            .exec(&db)
            .await
            .expect("delete test history rows");
        gateway_postgres::entities::custom_indicator::Entity::delete_by_id(indicator_id)
            .exec(&db)
            .await
            .expect("delete test indicator");

        let name_response = if code_was_first {
            expected_response(expected_definition(
                renamed,
                changed_code,
                json!({"type": "object"}),
                json!({"type": "object"}),
                json!(null),
            ))
        } else {
            expected_response(expected_definition(
                renamed,
                initial_code,
                json!({"type": "object"}),
                json!({"type": "object"}),
                json!(null),
            ))
        };
        let code_response = if code_was_first {
            expected_response(expected_definition(
                &initial_name,
                changed_code,
                json!({"type": "object"}),
                json!({"type": "object"}),
                json!(null),
            ))
        } else {
            expected_response(expected_definition(
                renamed,
                changed_code,
                json!({"type": "object"}),
                json!({"type": "object"}),
                json!(null),
            ))
        };
        let first_after = if code_was_first {
            expected_definition(
                &initial_name,
                changed_code,
                json!({"type": "object"}),
                json!({"type": "object"}),
                json!(null),
            )
        } else {
            expected_definition(
                renamed,
                initial_code,
                json!({"type": "object"}),
                json!({"type": "object"}),
                json!(null),
            )
        };
        let second_before = first_after.clone();
        let final_definition = expected_definition(
            renamed,
            changed_code,
            json!({"type": "object"}),
            json!({"type": "object"}),
            json!(null),
        );

        assert_eq!(
            (
                name_status,
                name_body,
                code_status,
                code_body,
                current_status,
                current_body,
                history_status,
                history_count,
                update_history,
                create_history,
            ),
            (
                StatusCode::OK,
                name_response,
                StatusCode::OK,
                code_response,
                StatusCode::OK,
                expected_response(final_definition.clone()),
                StatusCode::OK,
                3,
                vec![
                    expected_history_row(
                        &id,
                        "update",
                        expected_snapshot(
                            Some(expected_definition(
                                &initial_name,
                                initial_code,
                                json!({"type": "object"}),
                                json!({"type": "object"}),
                                json!(null),
                            )),
                            Some(first_after)
                        ),
                    ),
                    expected_history_row(
                        &id,
                        "update",
                        expected_snapshot(Some(second_before), Some(final_definition)),
                    ),
                ],
                vec![expected_history_row(
                    &id,
                    "create",
                    expected_snapshot(
                        None,
                        Some(expected_definition(
                            &initial_name,
                            initial_code,
                            json!({"type": "object"}),
                            json!({"type": "object"}),
                            json!(null),
                        )),
                    ),
                )],
            ),
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
