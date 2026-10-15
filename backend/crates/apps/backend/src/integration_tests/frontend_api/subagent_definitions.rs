#[cfg(test)]
mod tests {
    use super::super::{assert_response_eq, create_agent_config};
    use crate::testing::create_test_server;
    use axum::http::StatusCode;
    use rstest::{fixture, rstest};
    use serde_json::{Value, json};

    #[fixture]
    async fn subagent_validation_server() -> axum_test::TestServer {
        let db = gateway_postgres::test_support::create_test_transaction().await;
        create_test_server(db).await
    }

    fn normalize(mut value: Value) -> Value {
        for key in ["id", "created_at", "updated_at"] {
            if let Some(value) = value.get_mut(key) {
                *value = Value::String(format!("<{key}>"));
            }
        }
        if let Some(items) = value.as_array_mut() {
            for item in items {
                *item = normalize(std::mem::take(item));
            }
        }
        value
    }

    fn create_body() -> Value {
        json!({
            "key": "researcher",
            "description": "Investigates a focused question",
            "model": "example-model-research",
            "reasoning_effort": "medium",
            "prompt": "Return findings with sources",
            "tools": ["search_web", "read_page"],
            "timeout_minutes": 5,
            "max_model_calls": 30,
            "tool_limits": { "search_web": 15, "read_page": 10 },
            "output": {
                "conclusion": { "type": "string" },
                "sources": { "type": "array" }
            }
        })
    }

    fn update_body() -> Value {
        json!({
            "description": "Checks a focused question",
            "model": "example-model-review",
            "reasoning_effort": "low",
            "prompt": "Return a checked result with sources",
            "tools": ["search_web"],
            "timeout_minutes": 8,
            "max_model_calls": 24,
            "tool_limits": { "search_web": 12 },
            "output": {
                "result": { "type": "string" }
            }
        })
    }

    fn expected_definition(mut fields: Value, key: &str) -> Value {
        fields["key"] = json!(key);
        fields["id"] = json!("<id>");
        fields["created_at"] = json!("<created_at>");
        fields["updated_at"] = json!("<updated_at>");
        fields
    }

    #[backend_test_macros::database_test]
    async fn subagent_definition_crud_round_trip(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let body = create_body();
        let created = server.post("/api/subagent-definitions").json(&body).await;
        let expected_created = expected_definition(body, "researcher");
        assert_eq!(
            (created.status_code(), normalize(created.json())),
            (StatusCode::CREATED, expected_created.clone()),
        );

        let listed = server.get("/api/subagent-definitions").await;
        assert_eq!(
            (listed.status_code(), normalize(listed.json())),
            (StatusCode::OK, json!([expected_created.clone()])),
        );

        let fetched = server.get("/api/subagent-definitions/researcher").await;
        assert_eq!(
            (fetched.status_code(), normalize(fetched.json())),
            (StatusCode::OK, expected_created),
        );

        let updated = server
            .put("/api/subagent-definitions/researcher")
            .json(&update_body())
            .await;
        let expected_updated = expected_definition(update_body(), "researcher");
        assert_eq!(
            (updated.status_code(), normalize(updated.json())),
            (StatusCode::OK, expected_updated),
        );

        let deleted = server.delete("/api/subagent-definitions/researcher").await;
        assert_response_eq(&deleted, StatusCode::NO_CONTENT, None);
        let missing = server.get("/api/subagent-definitions/researcher").await;
        assert_response_eq(
            &missing,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": "subagent definition 'researcher' not found" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn duplicate_subagent_definition_key_returns_409(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let body = create_body();
        let created = server.post("/api/subagent-definitions").json(&body).await;
        created.assert_status(StatusCode::CREATED);

        let duplicate = server.post("/api/subagent-definitions").json(&body).await;
        assert_response_eq(
            &duplicate,
            StatusCode::CONFLICT,
            Some(json!({ "error": "subagent definition with key 'researcher' already exists" })),
        );
    }

    #[rstest]
    #[case::invalid_key(false, "key", json!("Researcher"), "subagent definition key must match ^[a-z0-9][a-z0-9_-]*$ (got 'Researcher')")]
    #[case::empty_model(false, "model", json!(""), "subagent definition model must not be empty")]
    #[case::update_rejects_empty_model(true, "model", json!(""), "subagent definition model must not be empty")]
    #[case::empty_prompt(false, "prompt", json!(""), "subagent definition prompt must not be empty")]
    #[case::update_rejects_empty_prompt(true, "prompt", json!(""), "subagent definition prompt must not be empty")]
    #[case::run_subagent_tool(false, "tools", json!(["run_subagent"]), "subagent definition tools must not include 'run_subagent'")]
    #[case::update_rejects_run_subagent_tool(true, "tools", json!(["run_subagent"]), "subagent definition tools must not include 'run_subagent'")]
    #[case::timeout_below_minimum(false, "timeout_minutes", json!(0), "subagent definition timeout_minutes must be between 1 and 360, got 0")]
    #[case::timeout_above_maximum(false, "timeout_minutes", json!(361), "subagent definition timeout_minutes must be between 1 and 360, got 361")]
    #[case::non_positive_model_calls(false, "max_model_calls", json!(0), "subagent definition max_model_calls must be greater than 0, got 0")]
    #[case::negative_tool_limit(false, "tool_limits", json!({ "search_web": -1 }), "subagent definition tool_limits must be non-negative (got search_web = -1)")]
    #[case::update_rejects_negative_tool_limit(true, "tool_limits", json!({ "search_web": -1 }), "subagent definition tool_limits must be non-negative (got search_web = -1)")]
    #[case::undeclared_tool_limit(false, "tool_limits", json!({ "unknown_tool": 1 }), "subagent definition tool_limits references undeclared tool 'unknown_tool'")]
    #[case::update_rejects_undeclared_tool_limit(true, "tool_limits", json!({ "unknown_tool": 1 }), "subagent definition tool_limits references undeclared tool 'unknown_tool'")]
    #[tokio::test]
    async fn rejects_invalid_subagent_definition_fields(
        #[future] subagent_validation_server: axum_test::TestServer,
        #[case] update: bool,
        #[case] field: &str,
        #[case] value: Value,
        #[case] expected_error: &str,
    ) {
        let server = subagent_validation_server.await;
        if update {
            let created = server
                .post("/api/subagent-definitions")
                .json(&create_body())
                .await;
            created.assert_status(StatusCode::CREATED);
        }

        let mut body = if update { update_body() } else { create_body() };
        body[field] = value;
        let response = if update {
            server
                .put("/api/subagent-definitions/researcher")
                .json(&body)
                .await
        } else {
            server.post("/api/subagent-definitions").json(&body).await
        };
        assert_response_eq(
            &response,
            StatusCode::BAD_REQUEST,
            Some(json!({ "error": expected_error })),
        );
    }

    #[backend_test_macros::database_test]
    async fn graph_rejects_unknown_subagent_and_referenced_definition_cannot_be_deleted(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;
        let create = server
            .post("/api/subagent-definitions")
            .json(&create_body())
            .await;
        create.assert_status(StatusCode::CREATED);

        let unknown_graph = indoc::indoc! {"
            phases:
              - key: research
                label: Research
                model: example-model-phase
                prompt: Investigate
                subagents: [missing]
                max_subagent_runs: 2
        "};
        let rejected = server
            .put("/api/agent-configs/explore/agent-graph")
            .json(&json!({ "content": unknown_graph }))
            .await;
        assert_response_eq(
            &rejected,
            StatusCode::BAD_REQUEST,
            Some(json!({
                "error": "subagent definition 'missing' referenced by agent_graph does not exist"
            })),
        );
        let unchanged = server.get("/api/agent-configs/explore/agent-graph").await;
        assert_response_eq(&unchanged, StatusCode::OK, Some(json!({ "content": "" })));

        let valid_graph = indoc::indoc! {"
            phases:
              - key: research
                label: Research
                model: example-model-phase
                prompt: Investigate
                subagents: [researcher]
                max_subagent_runs: 2
        "};
        let saved = server
            .put("/api/agent-configs/explore/agent-graph")
            .json(&json!({ "content": valid_graph }))
            .await;
        assert_response_eq(
            &saved,
            StatusCode::OK,
            Some(json!({ "content": valid_graph })),
        );

        let deletion = server.delete("/api/subagent-definitions/researcher").await;
        assert_response_eq(
            &deletion,
            StatusCode::CONFLICT,
            Some(json!({
                "error": "subagent definition 'researcher' is referenced by agent_config 'explore' phase 'research'"
            })),
        );
    }
}
