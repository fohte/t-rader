#[cfg(test)]
mod tests {
    use crate::testing::create_test_server;
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    fn normalize(mut value: Value) -> Value {
        for key in ["id", "created_at", "updated_at"] {
            if let Some(v) = value.get_mut(key) {
                *v = Value::String(format!("<{key}>"));
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
        created.assert_status(StatusCode::CREATED);
        assert_eq!(
            normalize(created.json()),
            json!({
                "id": "<id>",
                "purpose": "explore",
                "agents_md": "",
                "skills": {},
                "agent_graph": "",
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
        );

        let list = server.get("/api/agent-configs").await;
        list.assert_status_ok();
        let body: Vec<Value> = list.json();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0]["purpose"], "explore");
    }

    #[backend_test_macros::database_test]
    async fn duplicate_purpose_is_409(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let body = json!({ "purpose": "explore" });
        server
            .post("/api/agent-configs")
            .json(&body)
            .await
            .assert_status(StatusCode::CREATED);
        let res = server.post("/api/agent-configs").json(&body).await;
        res.assert_status(StatusCode::CONFLICT);
    }

    #[backend_test_macros::database_test]
    async fn invalid_purpose_is_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "Bad Purpose" }))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[backend_test_macros::database_test]
    async fn get_nonexistent_agent_config_returns_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server.get("/api/agent-configs/missing").await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn delete_agent_config_removes_row(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await
            .assert_status(StatusCode::CREATED);

        server
            .delete("/api/agent-configs/explore")
            .await
            .assert_status(StatusCode::NO_CONTENT);
        server
            .get("/api/agent-configs/explore")
            .await
            .assert_status(StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn put_then_get_agents_md_round_trips(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await
            .assert_status(StatusCode::CREATED);

        let body = "# 方針\n慎重に運用する";
        let put = server
            .put("/api/agent-configs/explore/agents-md")
            .json(&json!({ "content": body }))
            .await;
        put.assert_status_ok();
        assert_eq!(put.json::<Value>(), json!({ "content": body }));

        let get = server.get("/api/agent-configs/explore/agents-md").await;
        get.assert_status_ok();
        assert_eq!(get.json::<Value>(), json!({ "content": body }));
    }

    #[backend_test_macros::database_test]
    async fn agents_md_get_404_for_unknown_purpose(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server.get("/api/agent-configs/missing/agents-md").await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn single_skill_add_update_delete_lifecycle(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await
            .assert_status(StatusCode::CREATED);

        server
            .put("/api/agent-configs/explore/skills/scout")
            .json(&json!({ "content": "first" }))
            .await
            .assert_status_ok();
        server
            .put("/api/agent-configs/explore/skills/scout")
            .json(&json!({ "content": "second" }))
            .await
            .assert_status_ok();
        server
            .put("/api/agent-configs/explore/skills/review")
            .json(&json!({ "content": "rev" }))
            .await
            .assert_status_ok();

        let after_adds = server.get("/api/agent-configs/explore/skills").await;
        assert_eq!(
            after_adds.json::<Value>(),
            json!({ "skills": { "scout": "second", "review": "rev" } }),
        );

        server
            .delete("/api/agent-configs/explore/skills/scout")
            .await
            .assert_status(StatusCode::NO_CONTENT);

        let after_del = server.get("/api/agent-configs/explore/skills").await;
        assert_eq!(
            after_del.json::<Value>(),
            json!({ "skills": { "review": "rev" } }),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_unknown_skill_returns_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await
            .assert_status(StatusCode::CREATED);
        let res = server
            .delete("/api/agent-configs/explore/skills/missing")
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn put_then_get_agent_graph_round_trips(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await
            .assert_status(StatusCode::CREATED);

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
        put.assert_status_ok();
        assert_eq!(put.json::<Value>(), json!({ "content": yaml }));

        let get = server.get("/api/agent-configs/explore/agent-graph").await;
        get.assert_status_ok();
        assert_eq!(get.json::<Value>(), json!({ "content": yaml }));
    }

    #[backend_test_macros::database_test]
    async fn put_agent_graph_rejects_invalid_yaml(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await
            .assert_status(StatusCode::CREATED);

        let res = server
            .put("/api/agent-configs/explore/agent-graph")
            .json(&json!({ "content": "phases: [" }))
            .await;
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[backend_test_macros::database_test]
    async fn get_agent_config_bundle_returns_agents_md_skills_and_agent_graph(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        server
            .post("/api/agent-configs")
            .json(&json!({ "purpose": "explore" }))
            .await
            .assert_status(StatusCode::CREATED);

        let agents_md = indoc::indoc! {"
            # 方針
            慎重に運用する"};
        server
            .put("/api/agent-configs/explore/agents-md")
            .json(&json!({ "content": agents_md }))
            .await
            .assert_status_ok();
        server
            .put("/api/agent-configs/explore/skills/scout")
            .json(&json!({ "content": "scout body" }))
            .await
            .assert_status_ok();

        let res = server.get("/api/agent-configs/explore/agent-config").await;
        res.assert_status_ok();
        assert_eq!(
            res.json::<Value>(),
            json!({
                "agents_md": agents_md,
                "skills": { "scout": "scout body" },
                "agent_graph": "",
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_agent_config_bundle_404_for_unknown_purpose(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server.get("/api/agent-configs/missing/agent-config").await;
        res.assert_status(StatusCode::NOT_FOUND);
    }
}
