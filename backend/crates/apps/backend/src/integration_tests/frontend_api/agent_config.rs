#[cfg(test)]
mod tests {
    use super::super::{assert_response_eq, create_agent_config};
    use crate::testing::create_test_server;
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    fn normalize(mut value: Value) -> Value {
        for key in ["id", "created_at", "updated_at"] {
            if let Some(v) = value.get_mut(key) {
                *v = Value::String(format!("<{key}>"));
            }
        }
        if let Some(items) = value.as_array_mut() {
            for item in items {
                *item = normalize(std::mem::take(item));
            }
        }
        value
    }

    #[backend_test_macros::database_test]
    async fn create_and_list_roundtrip(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let created = server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await;
        assert_eq!(
            (created.status_code(), normalize(created.json())),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "purpose": "explore",
                    "agents_md": "",
                    "skills": {},
                    "agent_graph": "",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );

        let list = server.get("/api/agent-configs").await;
        assert_eq!(
            (list.status_code(), normalize(list.json())),
            (
                StatusCode::OK,
                json!([{
                    "id": "<id>",
                    "purpose": "explore",
                    "agents_md": "",
                    "skills": {},
                    "agent_graph": "",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }]),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn duplicate_purpose_is_409(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let body = json!({ "purpose": "explore" });
        create_agent_config(&server, "explore").await;
        let res = server.post("/api/agent-configs").json(&body).await;
        assert_response_eq(
            &res,
            StatusCode::CONFLICT,
            Some(json!({ "error": "agent_config with purpose 'explore' already exists" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn invalid_purpose_is_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "Bad Purpose" }))
            .await;
        assert_response_eq(
            &res,
            StatusCode::BAD_REQUEST,
            Some(
                json!({ "error": "purpose must match ^[a-z0-9][a-z0-9_-]*$ (got 'Bad Purpose')" }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_nonexistent_agent_config_returns_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server.get("/api/agent-configs/missing").await;
        assert_response_eq(
            &res,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": "agent_config 'missing' not found" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_agent_config_removes_row(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;

        let deleted = server.delete("/api/agent-configs/explore").await;
        assert_response_eq(&deleted, StatusCode::NO_CONTENT, None);
        let missing = server.get("/api/agent-configs/explore").await;
        assert_response_eq(
            &missing,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": "agent_config 'explore' not found" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_then_get_agents_md_round_trips(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;

        let body = "# 方針\n慎重に運用する";
        let put = server
            .put("/api/agent-configs/explore/agents-md")
            .json(&json!({ "content": body }))
            .await;
        assert_response_eq(&put, StatusCode::OK, Some(json!({ "content": body })));

        let get = server.get("/api/agent-configs/explore/agents-md").await;
        assert_response_eq(&get, StatusCode::OK, Some(json!({ "content": body })));
    }

    #[backend_test_macros::database_test]
    async fn agents_md_get_404_for_unknown_purpose(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server.get("/api/agent-configs/missing/agents-md").await;
        assert_response_eq(
            &res,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": "agent_config 'missing' not found" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn single_skill_add_update_delete_lifecycle(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;

        let first = server
            .put("/api/agent-configs/explore/skills/scout")
            .json(&json!({ "content": "first" }))
            .await;
        assert_response_eq(&first, StatusCode::OK, Some(json!({ "content": "first" })));
        let second = server
            .put("/api/agent-configs/explore/skills/scout")
            .json(&json!({ "content": "second" }))
            .await;
        assert_response_eq(
            &second,
            StatusCode::OK,
            Some(json!({ "content": "second" })),
        );
        let review = server
            .put("/api/agent-configs/explore/skills/review")
            .json(&json!({ "content": "rev" }))
            .await;
        assert_response_eq(&review, StatusCode::OK, Some(json!({ "content": "rev" })));

        let after_adds = server.get("/api/agent-configs/explore/skills").await;
        assert_response_eq(
            &after_adds,
            StatusCode::OK,
            Some(json!({ "skills": { "scout": "second", "review": "rev" } })),
        );

        let deleted = server
            .delete("/api/agent-configs/explore/skills/scout")
            .await;
        assert_response_eq(&deleted, StatusCode::NO_CONTENT, None);

        let after_del = server.get("/api/agent-configs/explore/skills").await;
        assert_response_eq(
            &after_del,
            StatusCode::OK,
            Some(json!({ "skills": { "review": "rev" } })),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_unknown_skill_returns_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;
        let res = server
            .delete("/api/agent-configs/explore/skills/missing")
            .await;
        assert_response_eq(
            &res,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": "skill 'missing' not found" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_then_get_agent_graph_round_trips(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;

        let yaml = indoc::indoc! {"
            phases:
              - key: plan
                label: 調査計画
                model: sample-model-plan
                prompt: 仮説を立てよ
        "};
        let put = server
            .put("/api/agent-configs/explore/agent-graph")
            .json(&json!({ "content": yaml }))
            .await;
        assert_response_eq(&put, StatusCode::OK, Some(json!({ "content": yaml })));

        let get = server.get("/api/agent-configs/explore/agent-graph").await;
        assert_response_eq(&get, StatusCode::OK, Some(json!({ "content": yaml })));
    }

    #[backend_test_macros::database_test]
    async fn put_agent_graph_rejects_invalid_yaml(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;

        let res = server
            .put("/api/agent-configs/explore/agent-graph")
            .json(&json!({ "content": "phases: [" }))
            .await;
        assert_response_eq(
            &res,
            StatusCode::BAD_REQUEST,
            Some(json!({
                "error": "agent_graph is not valid YAML: did not find expected node content at line 2 column 1, while parsing a flow node"
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_agent_config_bundle_returns_agents_md_skills_and_agent_graph(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        create_agent_config(&server, "explore").await;

        let agents_md = indoc::indoc! {"
            # 方針
            慎重に運用する"};
        let put_agents_md = server
            .put("/api/agent-configs/explore/agents-md")
            .json(&json!({ "content": agents_md }))
            .await;
        assert_response_eq(
            &put_agents_md,
            StatusCode::OK,
            Some(json!({ "content": agents_md })),
        );
        let put_skill = server
            .put("/api/agent-configs/explore/skills/scout")
            .json(&json!({ "content": "scout body" }))
            .await;
        assert_response_eq(
            &put_skill,
            StatusCode::OK,
            Some(json!({ "content": "scout body" })),
        );

        let res = server.get("/api/agent-configs/explore/agent-config").await;
        assert_response_eq(
            &res,
            StatusCode::OK,
            Some(json!({
                "agents_md": agents_md,
                "skills": { "scout": "scout body" },
                "agent_graph": "",
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_agent_config_bundle_404_for_unknown_purpose(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server.get("/api/agent-configs/missing/agent-config").await;
        assert_response_eq(
            &res,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": "agent_config 'missing' not found" })),
        );
    }
}
