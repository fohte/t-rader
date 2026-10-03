#[cfg(test)]
mod tests {
    use super::super::{assert_response_eq, normalize_timestamps};
    use std::sync::Arc;

    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::EntityTrait;
    use serde_json::json;
    use uuid::Uuid;

    use crate::testing::agent_config;
    use crate::testing::{
        create_test_server, create_test_server_with_db,
        create_test_server_with_db_and_agent_client, insert_test_strategy,
        insert_test_strategy_task,
    };
    use core_application::agent_task_client::{
        AgentTaskError, FakeAgentTaskClient, SharedAgentTaskClient,
    };
    use core_application::strategy_task::DEFAULT_PURPOSE;
    use gateway_postgres::entities::{note_version, strategy_task, strategy_task_step};

    #[backend_test_macros::database_test]
    async fn submit_chat_creates_task_row_and_submits_to_agent(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_next_task_id("agent-task-1").await;
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "long").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": " inspect demo-code " }))
            .await;
        let mut body: serde_json::Value = res.json();
        let task_id = Uuid::parse_str(body["task_id"].as_str().expect("task_id")).expect("uuid");
        body["task_id"] = json!("<uuid>");
        assert_eq!(
            (res.status_code(), body),
            (
                axum::http::StatusCode::ACCEPTED,
                json!({
                    "task_id": "<uuid>",
                    "a2a_task_id": "agent-task-1",
                }),
            ),
        );

        let row = strategy_task::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .unwrap()
            .expect("row");
        let row_summary = (
            row.task_id,
            row.strategy_id,
            row.a2a_task_id,
            row.source,
            row.prompt,
            row.phase,
            row.error_summary,
        );
        assert_eq!(
            row_summary,
            (
                task_id,
                strategy_id,
                Some("agent-task-1".to_string()),
                "frontend".to_string(),
                "inspect demo-code".to_string(),
                gateway_postgres::entities::sea_orm_active_enums::StrategyTaskPhase::Running,
                None,
            ),
        );

        let submitted: Vec<(Uuid, String)> = fake
            .submitted
            .lock()
            .await
            .iter()
            .map(|s| (s.strategy_id, s.prompt.clone()))
            .collect();
        assert_eq!(
            submitted,
            vec![(strategy_id, "inspect demo-code".to_string())]
        );
    }

    #[backend_test_macros::database_test]
    async fn submit_chat_uses_requested_purpose(db: gateway_postgres::DatabaseHandle) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_next_task_id("purpose-task").await;
        let agent_client: SharedAgentTaskClient = fake.clone();
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "example-strategy").await;
        agent_config::create(&db, "example-purpose".to_string())
            .await
            .expect("insert test agent_config");

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": "inspect the example", "purpose": "example-purpose" }))
            .await;
        let status = res.status_code();
        let mut body: serde_json::Value = res.json();
        let task_id = Uuid::parse_str(body["task_id"].as_str().expect("task_id")).expect("uuid");
        body["task_id"] = json!("<uuid>");

        let row = strategy_task::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .unwrap()
            .expect("row");
        let submitted_purpose = fake
            .submitted
            .lock()
            .await
            .first()
            .map(|submitted| submitted.purpose.clone());

        assert_eq!(
            (status, body, row.purpose, submitted_purpose),
            (
                axum::http::StatusCode::ACCEPTED,
                json!({
                    "task_id": "<uuid>",
                    "a2a_task_id": "purpose-task",
                }),
                Some("example-purpose".to_string()),
                Some(Some("example-purpose".to_string())),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn submit_chat_unknown_strategy_returns_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/strategies/00000000-0000-0000-0000-000000000000/chat")
            .json(&json!({ "prompt": "x" }))
            .await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::NOT_FOUND,
            Some(json!({
                "error": "strategy 00000000-0000-0000-0000-000000000000 not found"
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn submit_chat_empty_prompt_returns_400(db: gateway_postgres::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "x").await;

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": "   " }))
            .await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::BAD_REQUEST,
            Some(json!({ "error": "prompt must not be empty" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn submit_chat_agent_not_configured_returns_503(db: gateway_postgres::DatabaseHandle) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_submit_error(AgentTaskError::NotConfigured).await;
        let agent_client: SharedAgentTaskClient = fake;
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "x").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");

        let res = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": "inspect demo-code" }))
            .await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Some(json!({ "error": "agent task client is not configured" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn submit_chat_missing_agent_config_uses_requested_purpose_for_status(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "example-strategy").await;
        let cases = [
            (
                json!({ "prompt": "inspect the example" }),
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "agent_config for purpose 'default' not found",
            ),
            (
                json!({ "prompt": "inspect the example", "purpose": "example-purpose" }),
                axum::http::StatusCode::BAD_REQUEST,
                "agent_config for purpose 'example-purpose' not found",
            ),
        ];

        for (body, expected_status, expected_error) in cases {
            let res = server
                .post(&format!("/api/strategies/{strategy_id}/chat"))
                .json(&body)
                .await;

            assert_eq!(
                (res.status_code(), res.json::<serde_json::Value>()),
                (expected_status, json!({ "error": expected_error })),
            );
        }
    }

    #[backend_test_macros::database_test]
    async fn get_strategy_task_returns_phase(db: gateway_postgres::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "x").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");

        let submit = server
            .post(&format!("/api/strategies/{strategy_id}/chat"))
            .json(&json!({ "prompt": "p" }))
            .await;
        let submit_body = submit.json::<serde_json::Value>();
        let task_id = submit_body["task_id"]
            .as_str()
            .map(|s| Uuid::parse_str(s).unwrap())
            .expect("task_id");
        let a2a_task_id = submit_body["a2a_task_id"]
            .as_str()
            .expect("a2a_task_id")
            .to_string();
        assert_eq!(
            (submit.status_code(), submit_body),
            (
                axum::http::StatusCode::ACCEPTED,
                json!({ "task_id": task_id, "a2a_task_id": a2a_task_id }),
            ),
        );

        let res = server
            .get(&format!("/api/strategies/{strategy_id}/tasks/{task_id}"))
            .await;
        let mut body: serde_json::Value = res.json();
        normalize_timestamps(&mut body);
        assert_eq!(
            (res.status_code(), body),
            (
                axum::http::StatusCode::OK,
                json!({
                    "task_id": task_id,
                    "strategy_id": strategy_id,
                    "a2a_task_id": a2a_task_id,
                    "source": "frontend",
                    "prompt": "p",
                    "phase": "running",
                    "error_summary": null,
                    "result_text": null,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "steps": [],
                    "purpose": "default",
                    "as_of": "<as_of>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_strategy_task_unknown_returns_404(db: gateway_postgres::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "x").await;

        let res = server
            .get(&format!(
                "/api/strategies/{strategy_id}/tasks/00000000-0000-0000-0000-000000000000"
            ))
            .await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::NOT_FOUND,
            Some(json!({
                "error": "strategy task 00000000-0000-0000-0000-000000000000 not found"
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_strategy_task_strategy_mismatch_returns_404(db: gateway_postgres::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_a = insert_test_strategy(&db, "a").await;
        let strategy_b = insert_test_strategy(&db, "b").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");

        let submit = server
            .post(&format!("/api/strategies/{strategy_a}/chat"))
            .json(&json!({ "prompt": "p" }))
            .await;
        let submit_body = submit.json::<serde_json::Value>();
        let task_id = submit_body["task_id"]
            .as_str()
            .map(|s| Uuid::parse_str(s).unwrap())
            .expect("task_id");
        let a2a_task_id = submit_body["a2a_task_id"]
            .as_str()
            .expect("a2a_task_id")
            .to_string();
        assert_eq!(
            (submit.status_code(), submit_body),
            (
                axum::http::StatusCode::ACCEPTED,
                json!({ "task_id": task_id, "a2a_task_id": a2a_task_id }),
            ),
        );

        let res = server
            .get(&format!("/api/strategies/{strategy_b}/tasks/{task_id}"))
            .await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::NOT_FOUND,
            Some(json!({ "error": format!("strategy task {task_id} not found") })),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_strategy_task_notes_returns_updated_notes_once_by_execution_step(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_a = insert_test_strategy(&db, "strategy-a").await;
        let strategy_b = insert_test_strategy(&db, "strategy-b").await;
        let base = chrono::Utc::now().fixed_offset();
        let task_id = insert_test_strategy_task(&db, strategy_a, "inspect", None, base).await;
        let other_task_id =
            insert_test_strategy_task(&db, strategy_a, "different task", None, base).await;
        let first_step_id = Uuid::new_v4();
        let second_step_id = Uuid::new_v4();
        let other_step_id = Uuid::new_v4();
        insert_test_task_step(&db, task_id, first_step_id, base, 0).await;
        insert_test_task_step(
            &db,
            task_id,
            second_step_id,
            base + chrono::Duration::seconds(1),
            1,
        )
        .await;
        insert_test_task_step(&db, other_task_id, other_step_id, base, 0).await;

        let updated_note_id = crate::testing::insert_test_note_with_execution_id(
            &db,
            strategy_b,
            "Updated title",
            "First body",
            &first_step_id.to_string(),
        )
        .await;
        let first_version = crate::testing::find_current_note_version(&db, updated_note_id)
            .await
            .expect("query current note version")
            .expect("current note version");
        note_version::ActiveModel {
            id: Set(first_version.id),
            is_current: Set(false),
            status: Set("superseded".to_string()),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("supersede first note version");
        let current_version_id = Uuid::new_v4();
        note_version::ActiveModel {
            id: Set(current_version_id),
            note_id: Set(updated_note_id),
            version_no: Set(2),
            title: Set("Updated title".to_string()),
            body_md: Set("Updated body".to_string()),
            frontmatter_json: Set(json!({})),
            graphs_json: Set(json!([])),
            status: Set("unread".to_string()),
            is_current: Set(true),
            change_reason: Set(Some("updated".to_string())),
            created_by_kind: Set("llm".to_string()),
            execution_id: Set(Some(second_step_id.to_string())),
            created_at: Set(base + chrono::Duration::seconds(2)),
            reviewed_at: Set(None),
        }
        .insert(&db)
        .await
        .expect("insert updated note version");

        crate::testing::insert_test_note_with_execution_id(
            &db,
            strategy_a,
            "Other task title",
            "Other task body",
            &other_step_id.to_string(),
        )
        .await;
        crate::testing::insert_test_note_in_scope(
            &db,
            Some(strategy_a),
            "Human title",
            "Human body",
        )
        .await;

        let response = server
            .get(&format!(
                "/api/strategies/{strategy_a}/tasks/{task_id}/notes"
            ))
            .await;
        let mut body: serde_json::Value = response.json();
        for note in body.as_array_mut().expect("response is a list") {
            normalize_timestamps(note);
            note["id"] = json!("<note-id>");
            note["version_id"] = json!("<version-id>");
        }

        assert_eq!(
            (response.status_code(), body),
            (
                axum::http::StatusCode::OK,
                json!([{
                    "id": "<note-id>",
                    "version_id": "<version-id>",
                    "version_no": 2,
                    "is_current": true,
                    "strategy_id": strategy_b.to_string(),
                    "title": "Updated title",
                    "body_md": "Updated body",
                    "frontmatter_json": {},
                    "kind": null,
                    "status": "unread",
                    "trigger": null,
                    "trigger_label": null,
                    "created_by_kind": "llm",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "graphs_json": [],
                    "execution_id": first_step_id.to_string(),
                }]),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_strategy_task_notes_strategy_mismatch_returns_404(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_a = insert_test_strategy(&db, "strategy-a").await;
        let strategy_b = insert_test_strategy(&db, "strategy-b").await;
        let created_at = chrono::Utc::now().fixed_offset();
        let task_id = insert_test_strategy_task(&db, strategy_a, "inspect", None, created_at).await;

        let response = server
            .get(&format!(
                "/api/strategies/{strategy_b}/tasks/{task_id}/notes"
            ))
            .await;

        assert_response_eq(
            &response,
            axum::http::StatusCode::NOT_FOUND,
            Some(json!({ "error": format!("strategy task {task_id} not found") })),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_strategy_tasks_returns_tasks_newest_first(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_id = insert_test_strategy(&db, "x").await;

        let base = chrono::Utc::now().fixed_offset();
        let task1 = insert_test_strategy_task(&db, strategy_id, "first", None, base).await;
        let task2 = insert_test_strategy_task(
            &db,
            strategy_id,
            "second",
            None,
            base + chrono::Duration::seconds(1),
        )
        .await;
        let task3 = insert_test_strategy_task(
            &db,
            strategy_id,
            "third",
            None,
            base + chrono::Duration::seconds(2),
        )
        .await;

        let res = server
            .get(&format!("/api/strategies/{strategy_id}/tasks"))
            .await;
        let mut body: Vec<serde_json::Value> = res.json();
        body.iter_mut().for_each(normalize_timestamps);
        assert_eq!(
            (res.status_code(), body),
            (
                axum::http::StatusCode::OK,
                vec![
                    json!({
                        "task_id": task3,
                        "strategy_id": strategy_id,
                        "source": "frontend",
                        "prompt": "third",
                        "phase": "completed",
                        "error_summary": null,
                        "created_at": "<created_at>",
                        "updated_at": "<updated_at>",
                        "purpose": null,
                        "as_of": "<as_of>",
                    }),
                    json!({
                        "task_id": task2,
                        "strategy_id": strategy_id,
                        "source": "frontend",
                        "prompt": "second",
                        "phase": "completed",
                        "error_summary": null,
                        "created_at": "<created_at>",
                        "updated_at": "<updated_at>",
                        "purpose": null,
                        "as_of": "<as_of>",
                    }),
                    json!({
                        "task_id": task1,
                        "strategy_id": strategy_id,
                        "source": "frontend",
                        "prompt": "first",
                        "phase": "completed",
                        "error_summary": null,
                        "created_at": "<created_at>",
                        "updated_at": "<updated_at>",
                        "purpose": null,
                        "as_of": "<as_of>",
                    }),
                ],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_strategy_tasks_unknown_strategy_returns_404(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000/tasks")
            .await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::NOT_FOUND,
            Some(json!({
                "error": "strategy 00000000-0000-0000-0000-000000000000 not found"
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_strategy_tasks_scoped_to_strategy(db: gateway_postgres::DatabaseHandle) {
        let (db, server) = create_test_server_with_db(db).await;
        let strategy_a = insert_test_strategy(&db, "a").await;
        let strategy_b = insert_test_strategy(&db, "b").await;

        let base = chrono::Utc::now().fixed_offset();
        let task_a = insert_test_strategy_task(&db, strategy_a, "for-a", None, base).await;
        insert_test_strategy_task(&db, strategy_b, "for-b", None, base).await;

        let res = server
            .get(&format!("/api/strategies/{strategy_a}/tasks"))
            .await;
        let mut body: Vec<serde_json::Value> = res.json();
        body.iter_mut().for_each(normalize_timestamps);
        assert_eq!(
            (res.status_code(), body),
            (
                axum::http::StatusCode::OK,
                vec![json!({
                    "task_id": task_a,
                    "strategy_id": strategy_a,
                    "source": "frontend",
                    "prompt": "for-a",
                    "phase": "completed",
                    "error_summary": null,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "purpose": null,
                    "as_of": "<as_of>",
                })],
            ),
        );
    }

    async fn insert_test_task_step(
        db: &gateway_postgres::DatabaseHandle,
        task_id: Uuid,
        execution_step_id: Uuid,
        started_at: chrono::DateTime<chrono::FixedOffset>,
        seq: i64,
    ) {
        strategy_task_step::ActiveModel {
            execution_step_id: Set(execution_step_id),
            task_id: Set(task_id),
            phase_key: Set("inspect".to_string()),
            label: Set("Inspect data".to_string()),
            model: Set("fixture-model".to_string()),
            status: Set(
                gateway_postgres::entities::sea_orm_active_enums::StrategyTaskStepStatus::Completed,
            ),
            item: Set(None),
            item_label: Set(None),
            output: Set(None),
            started_at: Set(started_at),
            finished_at: Set(Some(started_at)),
            trace_id: Set("fixture-trace".to_string()),
            span_id: Set("fixture-span".to_string()),
            error: Set(None),
            seq: Set(seq),
        }
        .insert(db)
        .await
        .expect("insert test strategy task step");
    }
}
