#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use chrono::{Duration, Utc};
    use core_application::{
        agent_task_client::{FakeAgentTaskClient, SharedAgentTaskClient},
        strategy_execution::{PayloadDefinition, PayloadKind},
        strategy_scope::StrategyScope,
        strategy_task::DEFAULT_PURPOSE,
    };
    use gateway_postgres::entities::{sea_orm_active_enums::StrategyTaskPhase, strategy_task};
    use indoc::indoc;
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::testing::{
        create_test_server_with_db_and_agent_client, insert_test_strategy,
        insert_test_strategy_task,
    };

    const GRAPH_AT_START: &str = indoc! {"
        phases:
          - key: stage-a
            label: Stage A
            kind: operation
            operation: evaluate_saved_code
            code_id: synthetic-code-a
    "};

    const GRAPH_AFTER_START: &str = indoc! {"
        phases:
          - key: stage-b
            label: Stage B
            kind: operation
            operation: evaluate_saved_code
            code_id: synthetic-code-b
    "};

    fn normalize_definition_response(body: &mut Value) {
        for snapshot in body["definition_snapshots"]
            .as_array_mut()
            .expect("definition snapshots")
        {
            snapshot["digest"] = json!("<digest>");
            snapshot["created_at"] = json!("<created_at>");
            snapshot["definition"]["agent_config"]["id"] = json!("<id>");
            snapshot["definition"]["agent_config"]["created_at"] = json!("<created_at>");
            snapshot["definition"]["agent_config"]["updated_at"] = json!("<updated_at>");
        }
    }

    fn expected_definition_response(task_id: Uuid) -> Value {
        json!({
            "task_id": task_id,
            "definition_snapshots": [{
                "task_id": task_id,
                "definition_key": "task_start",
                "definition": {
                    "agent_config": {
                        "id": "<id>",
                        "purpose": DEFAULT_PURPOSE,
                        "agents_md": "",
                        "skills": {},
                        "agent_graph": GRAPH_AT_START,
                        "created_at": "<created_at>",
                        "updated_at": "<updated_at>",
                    },
                    "subagent_definitions": [],
                },
                "digest": "<digest>",
                "created_at": "<created_at>",
            }],
        })
    }

    async fn submit_task(
        server: &axum_test::TestServer,
        client: &FakeAgentTaskClient,
        strategy_id: Uuid,
        a2a_task_id: &str,
    ) -> (StatusCode, Uuid) {
        client.set_next_task_id(a2a_task_id).await;
        let response = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": "review synthetic input" }))
            .await;
        let body: Value = response.json();
        let task_id =
            Uuid::parse_str(body["task_id"].as_str().expect("task id")).expect("task id is a UUID");
        (response.status_code(), task_id)
    }

    async fn set_task_state(
        db: &gateway_postgres::DatabaseHandle,
        task_id: Uuid,
        phase: StrategyTaskPhase,
        deadline_at: chrono::DateTime<chrono::FixedOffset>,
    ) {
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            phase: Set(phase),
            deadline_at: Set(deadline_at),
            ..Default::default()
        }
        .update(db)
        .await
        .expect("update task state");
    }

    #[backend_test_macros::database_test]
    async fn task_definition_read_uses_the_fixed_snapshot_for_finished_failed_and_expired_tasks(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let fake = std::sync::Arc::new(FakeAgentTaskClient::new());
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "synthetic-strategy").await;
        let other_strategy_id = insert_test_strategy(&db, "other-synthetic-strategy").await;
        crate::testing::agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert synthetic agent config");
        server
            .put(&format!("/api/agent-configs/{DEFAULT_PURPOSE}/agent-graph"))
            .json(&json!({ "content": GRAPH_AT_START }))
            .await;

        let tasks = ["synthetic-task-a", "synthetic-task-b", "synthetic-task-c"];
        let mut created = Vec::new();
        for a2a_task_id in tasks {
            created.push(submit_task(&server, &fake, strategy_id, a2a_task_id).await);
        }
        server
            .put(&format!("/api/agent-configs/{DEFAULT_PURPOSE}/agent-graph"))
            .json(&json!({ "content": GRAPH_AFTER_START }))
            .await;

        let now = Utc::now().fixed_offset();
        for ((_, task_id), phase, deadline) in [
            (
                created[0],
                StrategyTaskPhase::Completed,
                now + Duration::minutes(5),
            ),
            (
                created[1],
                StrategyTaskPhase::Failed,
                now + Duration::minutes(5),
            ),
            (
                created[2],
                StrategyTaskPhase::Running,
                now - Duration::minutes(1),
            ),
        ] {
            set_task_state(&db, task_id, phase, deadline).await;
        }

        let mut reads = Vec::new();
        for (_, task_id) in &created {
            let response = server
                .get(&format!(
                    "/api/strategies/{strategy_id}/tasks/{task_id}/execution-artifacts"
                ))
                .await;
            let mut body: Value = response.json();
            normalize_definition_response(&mut body);
            reads.push((response.status_code(), body));
        }
        let mismatch = server
            .get(&format!(
                "/api/strategies/{other_strategy_id}/tasks/{}/execution-artifacts",
                created[0].1
            ))
            .await;
        let mismatch_body: Value = mismatch.json();
        let submissions = created
            .iter()
            .map(|(status, task_id)| (*status, *task_id))
            .collect::<Vec<_>>();

        assert_eq!(
            (submissions, reads, mismatch.status_code(), mismatch_body),
            (
                vec![
                    (StatusCode::ACCEPTED, created[0].1),
                    (StatusCode::ACCEPTED, created[1].1),
                    (StatusCode::ACCEPTED, created[2].1),
                ],
                created
                    .iter()
                    .map(|(_, task_id)| {
                        (StatusCode::OK, expected_definition_response(*task_id))
                    })
                    .collect::<Vec<_>>(),
                StatusCode::NOT_FOUND,
                json!({ "error": "task execution artifact not found" }),
            ),
        );
    }

    fn normalize_payload_response(body: &mut Value) {
        body["manifest"]["created_at"] = json!("<created_at>");
        if !body["manifest"]["sealed_at"].is_null() {
            body["manifest"]["sealed_at"] = json!("<sealed_at>");
        }
        if !body["manifest"]["digest"].is_null() {
            body["manifest"]["digest"] = json!("<digest>");
        }
        if body["manifest"]["byte_count"] != 0 {
            body["manifest"]["byte_count"] = json!("<byte_count>");
        }
        if body["page"].is_object() {
            body["page"]["digest"] = json!("<digest>");
        }
    }

    #[backend_test_macros::database_test]
    async fn payload_reads_are_task_scoped_bounded_and_hide_unsealed_content(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db_and_agent_client(
            db,
            std::sync::Arc::new(FakeAgentTaskClient::new()),
        )
        .await;
        let strategy_id = insert_test_strategy(&db, "synthetic-strategy").await;
        let other_strategy_id = insert_test_strategy(&db, "other-synthetic-strategy").await;
        let now = Utc::now().fixed_offset();
        let task_id =
            insert_test_strategy_task(&db, strategy_id, "synthetic prompt", None, now).await;
        let a2a_task_id = "synthetic-task-payload";
        set_task_state(
            &db,
            task_id,
            StrategyTaskPhase::Running,
            now + Duration::minutes(5),
        )
        .await;
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            a2a_task_id: Set(Some(a2a_task_id.to_string())),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("set A2A task ID");

        let use_cases =
            crate::services::use_cases::build_use_cases_without_agent_task_client(db.clone())
                .strategy_executions();
        let access = use_cases
            .authorize_execution(StrategyScope::from(strategy_id), a2a_task_id, None)
            .await
            .expect("authorize running task for fixture setup");
        let sealed = use_cases
            .write_payload(
                &access,
                PayloadDefinition {
                    kind: PayloadKind::Output,
                    source: "synthetic-source".into(),
                    source_revision: Some("synthetic-revision".into()),
                    source_locator: None,
                    source_metadata: json!({}),
                    schema: json!({"type": "array"}),
                },
                json!([
                    {"value": 1},
                    {"value": 2},
                    {"value": 3},
                ]),
            )
            .await
            .expect("write sealed synthetic payload");
        let incomplete_id = use_cases
            .begin_payload(
                &access,
                PayloadDefinition {
                    kind: PayloadKind::Input,
                    source: "synthetic-source".into(),
                    source_revision: Some("synthetic-revision".into()),
                    source_locator: None,
                    source_metadata: json!({}),
                    schema: json!({"type": "array"}),
                },
                true,
            )
            .await
            .expect("begin incomplete synthetic payload");
        let incomplete_write = use_cases
            .append_payload_chunk(
                &access,
                incomplete_id,
                Some("different-revision"),
                json!([{"value": 4}]),
            )
            .await
            .map(|()| "ok".to_string())
            .unwrap_or_else(|error| error.to_string());
        set_task_state(
            &db,
            task_id,
            StrategyTaskPhase::Failed,
            now - Duration::minutes(1),
        )
        .await;
        let other_task_id =
            insert_test_strategy_task(&db, strategy_id, "other synthetic prompt", None, now).await;

        let sealed_response = server
            .get(&format!(
                "/api/strategies/{strategy_id}/tasks/{task_id}/execution-artifacts/payloads/{}?cursor=1&limit=1",
                sealed.id
            ))
            .await;
        let mut sealed_body: Value = sealed_response.json();
        normalize_payload_response(&mut sealed_body);
        let incomplete_response = server
            .get(&format!(
                "/api/strategies/{strategy_id}/tasks/{task_id}/execution-artifacts/payloads/{incomplete_id}"
            ))
            .await;
        let mut incomplete_body: Value = incomplete_response.json();
        normalize_payload_response(&mut incomplete_body);
        let cross_task_response = server
            .get(&format!(
                "/api/strategies/{strategy_id}/tasks/{other_task_id}/execution-artifacts/payloads/{}",
                sealed.id
            ))
            .await;
        let cross_task_body: Value = cross_task_response.json();
        let cross_strategy_response = server
            .get(&format!(
                "/api/strategies/{other_strategy_id}/tasks/{task_id}/execution-artifacts/payloads/{}",
                sealed.id
            ))
            .await;
        let cross_strategy_body: Value = cross_strategy_response.json();
        let invalid_limit_response = server
            .get(&format!(
                "/api/strategies/{strategy_id}/tasks/{task_id}/execution-artifacts/payloads/{}?limit=0",
                sealed.id
            ))
            .await;
        let invalid_limit_body: Value = invalid_limit_response.json();

        assert_eq!(
            (
                incomplete_write,
                sealed_response.status_code(),
                sealed_body,
                incomplete_response.status_code(),
                incomplete_body,
                cross_task_response.status_code(),
                cross_task_body,
                cross_strategy_response.status_code(),
                cross_strategy_body,
                invalid_limit_response.status_code(),
                invalid_limit_body,
            ),
            (
                "payload source revision changed between pages".to_string(),
                StatusCode::OK,
                json!({
                    "manifest": {
                        "id": sealed.id,
                        "task_id": task_id,
                        "execution_step_id": null,
                        "kind": "output",
                        "source": "synthetic-source",
                        "source_revision": "synthetic-revision",
                        "source_locator": null,
                        "source_metadata": {},
                        "schema": {"type": "array"},
                        "capture_metadata": null,
                        "root_is_array": true,
                        "chunk_count": 1,
                        "item_count": 3,
                        "byte_count": "<byte_count>",
                        "digest": "<digest>",
                        "status": "sealed",
                        "incomplete_reason": null,
                        "created_at": "<created_at>",
                        "sealed_at": "<sealed_at>",
                    },
                    "page": {
                        "payload_id": sealed.id,
                        "pointer": "",
                        "data": [{"value": 2}],
                        "cursor": 1,
                        "next_cursor": 2,
                        "total_count": 3,
                        "digest": "<digest>",
                    },
                }),
                StatusCode::OK,
                json!({
                    "manifest": {
                        "id": incomplete_id,
                        "task_id": task_id,
                        "execution_step_id": null,
                        "kind": "input",
                        "source": "synthetic-source",
                        "source_revision": "synthetic-revision",
                        "source_locator": null,
                        "source_metadata": {},
                        "schema": {"type": "array"},
                        "capture_metadata": null,
                        "root_is_array": true,
                        "chunk_count": 0,
                        "item_count": 0,
                        "byte_count": 0,
                        "digest": null,
                        "status": "incomplete",
                        "incomplete_reason": "source_revision_changed",
                        "created_at": "<created_at>",
                        "sealed_at": null,
                    },
                    "page": null,
                }),
                StatusCode::NOT_FOUND,
                json!({ "error": "task execution artifact not found" }),
                StatusCode::NOT_FOUND,
                json!({ "error": "task execution artifact not found" }),
                StatusCode::BAD_REQUEST,
                json!({ "error": "invalid execution payload page request" }),
            ),
        );
    }
}
