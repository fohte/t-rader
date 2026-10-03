#[cfg(test)]
mod tests {
    use super::super::{assert_response_eq, create_agent_config, create_strategy};
    use crate::testing::create_test_server;
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    /// trigger response の時刻系フィールドを placeholder に正規化する。
    /// 全フィールドを 1 つの literal と equality で比較するためのヘルパ。
    /// `trigger_id` は呼び出し側が事前生成 ID を知っているケースとそうでないケースの両方が
    /// あるため、呼び出し側で `preserve_trigger_id=false` を指定したときだけ placeholder 化する。
    fn normalize_trigger(mut value: Value, preserve_trigger_id: bool) -> Value {
        if !preserve_trigger_id && let Some(v) = value.get_mut("trigger_id") {
            *v = Value::String("<trigger_id>".into());
        }
        for key in ["created_at", "updated_at"] {
            if let Some(v) = value.get_mut(key) {
                *v = Value::String(format!("<{key}>"));
            }
        }
        if let Some(v) = value.get_mut("last_fired_at")
            && !v.is_null()
        {
            *v = Value::String("<last_fired_at>".into());
        }
        value
    }

    async fn create_hook_trigger_with_event_match(
        db: gateway_postgres::DatabaseHandle,
    ) -> (axum_test::TestServer, String, String) {
        let server = create_test_server(db).await;
        let sid = create_strategy(&server, "s").await;
        let created = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "hook",
                "hook_slug": "sample-hook",
                "event_match": {"event": {"eq": "initial"}},
                "prompt_template": "x",
            }))
            .await;
        let body = created.json::<Value>();
        let tid = body["trigger_id"].as_str().unwrap().to_string();
        let body = normalize_trigger(body, false);
        assert_eq!(
            (created.status_code(), body),
            (
                StatusCode::CREATED,
                json!({
                    "trigger_id": "<trigger_id>",
                    "strategy_id": sid,
                    "purpose": null,
                    "kind": "hook",
                    "schedule": null,
                    "hook_slug": "sample-hook",
                    "event_match": {"event": {"eq": "initial"}},
                    "prompt_template": "x",
                    "enabled": true,
                    "last_fired_at": null,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
        (server, sid, tid)
    }

    #[backend_test_macros::database_test]
    async fn create_cron_trigger_succeeds(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let sid = create_strategy(&server, "s").await;
        create_agent_config(&server, "synthetic-purpose").await;
        let res = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "cron",
                "purpose": "synthetic-purpose",
                "schedule": "0 9 * * 1-5",
                "prompt_template": "morning briefing for {{strategy.name}}",
            }))
            .await;
        assert_eq!(
            (res.status_code(), normalize_trigger(res.json(), false)),
            (
                StatusCode::CREATED,
                json!({
                    "trigger_id": "<trigger_id>",
                    "strategy_id": sid,
                    "purpose": "synthetic-purpose",
                    "kind": "cron",
                    "schedule": "0 9 * * 1-5",
                    "hook_slug": null,
                    "event_match": null,
                    "prompt_template": "morning briefing for {{strategy.name}}",
                    "enabled": true,
                    "last_fired_at": null,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_hook_trigger_succeeds(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let sid = create_strategy(&server, "s").await;
        let res = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "hook",
                "hook_slug": "tv-alert",
                "event_match": {"event": {"eq": "fired"}},
                "prompt_template": "alert: {{payload.symbol}}",
            }))
            .await;
        assert_eq!(
            (res.status_code(), normalize_trigger(res.json(), false)),
            (
                StatusCode::CREATED,
                json!({
                    "trigger_id": "<trigger_id>",
                    "strategy_id": sid,
                    "purpose": null,
                    "kind": "hook",
                    "schedule": null,
                    "hook_slug": "tv-alert",
                    "event_match": {"event": {"eq": "fired"}},
                    "prompt_template": "alert: {{payload.symbol}}",
                    "enabled": true,
                    "last_fired_at": null,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_cron_without_schedule_is_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let sid = create_strategy(&server, "s").await;
        let res = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({"kind": "cron", "prompt_template": "x"}))
            .await;
        assert_response_eq(
            &res,
            StatusCode::BAD_REQUEST,
            Some(json!({ "error": "schedule is required for kind=cron" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_hook_with_schedule_is_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let sid = create_strategy(&server, "s").await;
        let res = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "hook",
                "hook_slug": "x",
                "schedule": "0 * * * *",
                "prompt_template": "x",
            }))
            .await;
        assert_response_eq(
            &res,
            StatusCode::BAD_REQUEST,
            Some(json!({ "error": "schedule must be omitted for kind=hook" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_for_missing_strategy_is_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/strategies/00000000-0000-0000-0000-000000000000/triggers")
            .json(&json!({
                "kind": "cron",
                "schedule": "* * * * *",
                "prompt_template": "x",
            }))
            .await;
        assert_response_eq(
            &res,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": "strategy 00000000-0000-0000-0000-000000000000 not found" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_with_invalid_purpose_returns_error(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let sid = create_strategy(&server, "s").await;
        let cases = [
            (
                "unknown purpose",
                "missing-purpose",
                StatusCode::NOT_FOUND,
                json!({ "error": "agent_config purpose missing-purpose not found" }),
            ),
            (
                "empty purpose",
                " ",
                StatusCode::BAD_REQUEST,
                json!({ "error": "purpose must not be empty" }),
            ),
        ];

        for (case, purpose, expected_status, expected_body) in cases {
            let res = server
                .post(&format!("/api/strategies/{sid}/triggers"))
                .json(&json!({
                    "kind": "cron",
                    "purpose": purpose,
                    "schedule": "0 9 * * *",
                    "prompt_template": "x",
                }))
                .await;

            assert_eq!(
                (res.status_code(), res.json::<Value>()),
                (expected_status, expected_body),
                "case: {case}",
            );
        }
    }

    #[backend_test_macros::database_test]
    async fn deleting_agent_config_resets_trigger_purpose_to_default(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let sid = create_strategy(&server, "s").await;
        create_agent_config(&server, "synthetic-purpose").await;
        let created: Value = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "cron",
                "purpose": "synthetic-purpose",
                "schedule": "0 9 * * *",
                "prompt_template": "x",
            }))
            .await
            .json();
        let tid = created["trigger_id"].as_str().unwrap().to_string();

        let deleted = server.delete("/api/agent-configs/synthetic-purpose").await;
        assert_response_eq(&deleted, StatusCode::NO_CONTENT, None);
        let trigger = server.get(&format!("/api/triggers/{tid}")).await;

        assert_eq!(
            (
                trigger.status_code(),
                normalize_trigger(trigger.json(), true)
            ),
            (
                StatusCode::OK,
                json!({
                    "trigger_id": tid,
                    "strategy_id": sid,
                    "purpose": null,
                    "kind": "cron",
                    "schedule": "0 9 * * *",
                    "hook_slug": null,
                    "event_match": null,
                    "prompt_template": "x",
                    "enabled": true,
                    "last_fired_at": null,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn duplicate_hook_slug_is_409(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let sid = create_strategy(&server, "s").await;
        let body = json!({
            "kind": "hook",
            "hook_slug": "dup",
            "prompt_template": "x",
        });
        let created = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&body)
            .await;
        assert_eq!(
            (
                created.status_code(),
                normalize_trigger(created.json(), false)
            ),
            (
                StatusCode::CREATED,
                json!({
                    "trigger_id": "<trigger_id>", "strategy_id": sid, "purpose": null,
                    "kind": "hook", "schedule": null, "hook_slug": "dup",
                    "event_match": null, "prompt_template": "x", "enabled": true,
                    "last_fired_at": null, "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
        let res = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&body)
            .await;
        assert_response_eq(
            &res,
            StatusCode::CONFLICT,
            Some(json!({ "error": "resource already exists" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_filters_by_kind(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let sid = create_strategy(&server, "s").await;
        let cron = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "cron",
                "schedule": "* * * * *",
                "prompt_template": "c",
            }))
            .await;
        assert_eq!(
            (cron.status_code(), normalize_trigger(cron.json(), false)),
            (
                StatusCode::CREATED,
                json!({
                    "trigger_id": "<trigger_id>", "strategy_id": sid, "purpose": null,
                    "kind": "cron", "schedule": "* * * * *", "hook_slug": null,
                    "event_match": null, "prompt_template": "c", "enabled": true,
                    "last_fired_at": null, "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
        let hook = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "hook",
                "hook_slug": "h",
                "prompt_template": "h",
            }))
            .await;
        assert_eq!(
            (hook.status_code(), normalize_trigger(hook.json(), false)),
            (
                StatusCode::CREATED,
                json!({
                    "trigger_id": "<trigger_id>", "strategy_id": sid, "purpose": null,
                    "kind": "hook", "schedule": null, "hook_slug": "h",
                    "event_match": null, "prompt_template": "h", "enabled": true,
                    "last_fired_at": null, "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );

        let cron_only = server
            .get(&format!("/api/strategies/{sid}/triggers?kind=cron"))
            .await;
        let body: Vec<Value> = cron_only.json();
        let normalized: Vec<Value> = body
            .into_iter()
            .map(|v| normalize_trigger(v, false))
            .collect();
        assert_eq!(
            (cron_only.status_code(), normalized),
            (
                StatusCode::OK,
                vec![json!({
                    "trigger_id": "<trigger_id>",
                    "strategy_id": sid,
                    "purpose": null,
                    "kind": "cron",
                    "schedule": "* * * * *",
                    "hook_slug": null,
                    "event_match": null,
                    "prompt_template": "c",
                    "enabled": true,
                    "last_fired_at": null,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                })],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_scoped_to_owning_strategy(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let s1 = create_strategy(&server, "a").await;
        let s2 = create_strategy(&server, "b").await;
        let created = server
            .post(&format!("/api/strategies/{s1}/triggers"))
            .json(&json!({"kind": "cron", "schedule": "* * * * *", "prompt_template": "x"}))
            .await;
        assert_eq!(
            (
                created.status_code(),
                normalize_trigger(created.json(), false)
            ),
            (
                StatusCode::CREATED,
                json!({
                    "trigger_id": "<trigger_id>", "strategy_id": s1, "purpose": null,
                    "kind": "cron", "schedule": "* * * * *", "hook_slug": null,
                    "event_match": null, "prompt_template": "x", "enabled": true,
                    "last_fired_at": null, "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
        let res = server.get(&format!("/api/strategies/{s2}/triggers")).await;
        assert_response_eq(&res, StatusCode::OK, Some(json!([])));
    }

    #[backend_test_macros::database_test]
    async fn get_update_delete_round_trip(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let sid = create_strategy(&server, "s").await;
        let created: Value = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "cron",
                "schedule": "0 9 * * 1-5",
                "prompt_template": "old",
            }))
            .await
            .json();
        let tid = created["trigger_id"].as_str().unwrap().to_string();

        let updated = server
            .put(&format!("/api/triggers/{tid}"))
            .json(&json!({
                "prompt_template": "new",
                "enabled": false,
            }))
            .await;
        let expected = json!({
            "trigger_id": tid,
            "strategy_id": sid,
            "purpose": null,
            "kind": "cron",
            "schedule": "0 9 * * 1-5",
            "hook_slug": null,
            "event_match": null,
            "prompt_template": "new",
            "enabled": false,
            "last_fired_at": null,
            "created_at": "<created_at>",
            "updated_at": "<updated_at>",
        });
        assert_eq!(
            (
                updated.status_code(),
                normalize_trigger(updated.json(), true)
            ),
            (StatusCode::OK, expected.clone()),
        );

        let got = server.get(&format!("/api/triggers/{tid}")).await;
        assert_eq!(
            (got.status_code(), normalize_trigger(got.json(), true)),
            (StatusCode::OK, expected),
        );

        let deleted = server.delete(&format!("/api/triggers/{tid}")).await;
        assert_response_eq(&deleted, StatusCode::NO_CONTENT, None);

        let after = server.get(&format!("/api/triggers/{tid}")).await;
        assert_response_eq(
            &after,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": format!("trigger {tid} not found") })),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_purpose_omission_null_and_value_have_expected_semantics(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let sid = create_strategy(&server, "s").await;
        create_agent_config(&server, "synthetic-purpose").await;
        create_agent_config(&server, "replacement-purpose").await;
        let created: Value = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "cron",
                "purpose": "synthetic-purpose",
                "schedule": "0 9 * * *",
                "prompt_template": "x",
            }))
            .await
            .json();
        let tid = created["trigger_id"].as_str().unwrap().to_string();
        let cases = [
            (json!({}), Some("synthetic-purpose")),
            (json!({ "purpose": null }), None),
            (
                json!({ "purpose": "replacement-purpose" }),
                Some("replacement-purpose"),
            ),
        ];

        for (body, purpose) in cases {
            let updated = server
                .put(&format!("/api/triggers/{tid}"))
                .json(&body)
                .await;
            assert_eq!(
                (
                    updated.status_code(),
                    normalize_trigger(updated.json(), true)
                ),
                (
                    StatusCode::OK,
                    json!({
                        "trigger_id": tid,
                        "strategy_id": sid,
                        "purpose": purpose,
                        "kind": "cron",
                        "schedule": "0 9 * * *",
                        "hook_slug": null,
                        "event_match": null,
                        "prompt_template": "x",
                        "enabled": true,
                        "last_fired_at": null,
                        "created_at": "<created_at>",
                        "updated_at": "<updated_at>",
                    }),
                ),
            );
        }
    }

    #[backend_test_macros::database_test]
    async fn update_event_match_omitted_keeps_existing_value(db: gateway_postgres::DatabaseHandle) {
        let (server, sid, tid) = create_hook_trigger_with_event_match(db).await;
        let updated = server
            .put(&format!("/api/triggers/{tid}"))
            .json(&json!({}))
            .await;
        assert_eq!(
            (
                updated.status_code(),
                normalize_trigger(updated.json(), true)
            ),
            (
                StatusCode::OK,
                json!({
                    "trigger_id": tid,
                    "strategy_id": sid,
                    "purpose": null,
                    "kind": "hook",
                    "schedule": null,
                    "hook_slug": "sample-hook",
                    "event_match": {"event": {"eq": "initial"}},
                    "prompt_template": "x",
                    "enabled": true,
                    "last_fired_at": null,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_event_match_null_clears_existing_value(db: gateway_postgres::DatabaseHandle) {
        let (server, sid, tid) = create_hook_trigger_with_event_match(db).await;
        let updated = server
            .put(&format!("/api/triggers/{tid}"))
            .json(&json!({ "event_match": null }))
            .await;
        assert_eq!(
            (
                updated.status_code(),
                normalize_trigger(updated.json(), true)
            ),
            (
                StatusCode::OK,
                json!({
                    "trigger_id": tid,
                    "strategy_id": sid,
                    "purpose": null,
                    "kind": "hook",
                    "schedule": null,
                    "hook_slug": "sample-hook",
                    "event_match": null,
                    "prompt_template": "x",
                    "enabled": true,
                    "last_fired_at": null,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_event_match_value_replaces_existing_value(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (server, sid, tid) = create_hook_trigger_with_event_match(db).await;
        let updated = server
            .put(&format!("/api/triggers/{tid}"))
            .json(&json!({ "event_match": {"source": {"eq": "replacement"}} }))
            .await;
        assert_eq!(
            (
                updated.status_code(),
                normalize_trigger(updated.json(), true)
            ),
            (
                StatusCode::OK,
                json!({
                    "trigger_id": tid,
                    "strategy_id": sid,
                    "purpose": null,
                    "kind": "hook",
                    "schedule": null,
                    "hook_slug": "sample-hook",
                    "event_match": {"source": {"eq": "replacement"}},
                    "prompt_template": "x",
                    "enabled": true,
                    "last_fired_at": null,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_hook_slug_on_cron_trigger_is_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let sid = create_strategy(&server, "s").await;
        let created: Value = server
            .post(&format!("/api/strategies/{sid}/triggers"))
            .json(&json!({
                "kind": "cron",
                "schedule": "* * * * *",
                "prompt_template": "x",
            }))
            .await
            .json();
        let tid = created["trigger_id"].as_str().unwrap().to_string();
        let res = server
            .put(&format!("/api/triggers/{tid}"))
            .json(&json!({"hook_slug": "x"}))
            .await;
        assert_response_eq(
            &res,
            StatusCode::BAD_REQUEST,
            Some(json!({ "error": "hook_slug can only be set when kind=hook" })),
        );
    }
}
