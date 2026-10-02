mod agent_tasks {
    use axum::http::StatusCode;
    use core_application::strategy_task::STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER;
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
    use serde_json::json;

    use crate::testing::{TEST_AGENT_WEBHOOK_TOKEN, create_test_server_with_state};

    const NOTIFICATION_TOKEN_HEADER: &str = "x-a2a-notification-token";

    async fn reconciliation_jobs(
        db: &gateway_postgres::DatabaseHandle,
    ) -> Vec<(String, String, String)> {
        let statement = Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT tasks.identifier, jobs.payload::text AS payload, jobs.key \
                 FROM graphile_worker._private_jobs AS jobs \
                 JOIN graphile_worker._private_tasks AS tasks ON tasks.id = jobs.task_id \
                 WHERE jobs.key = '{STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER}'"
            ),
        );
        db.query_all_raw(statement)
            .await
            .expect("read reconciliation jobs")
            .into_iter()
            .map(|row| {
                (
                    row.try_get::<String>("", "identifier")
                        .expect("job identifier"),
                    row.try_get::<String>("", "payload").expect("job payload"),
                    row.try_get::<String>("", "key").expect("job key"),
                )
            })
            .collect()
    }

    #[backend_test_macros::database_test]
    async fn valid_token_returns_204_and_enqueues_reconciliation(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (_, server) = create_test_server_with_state(db.clone()).await;

        let res = server
            .post("/api/agent-tasks/notifications")
            .add_header(NOTIFICATION_TOKEN_HEADER, TEST_AGENT_WEBHOOK_TOKEN)
            .json(&json!({"id": "task-1", "status": {"state": "completed"}}))
            .await;
        assert_eq!(
            (res.status_code(), reconciliation_jobs(&db).await),
            (
                StatusCode::NO_CONTENT,
                vec![(
                    STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER.to_string(),
                    "{}".to_string(),
                    STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER.to_string(),
                )],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn mismatched_token_returns_401(db: gateway_postgres::DatabaseHandle) {
        let (_, server) = create_test_server_with_state(db.clone()).await;

        let res = server
            .post("/api/agent-tasks/notifications")
            .add_header(NOTIFICATION_TOKEN_HEADER, "wrong-token")
            .json(&json!({"id": "task-1"}))
            .await;
        assert_eq!(
            (res.status_code(), reconciliation_jobs(&db).await),
            (StatusCode::UNAUTHORIZED, vec![]),
        );
    }

    #[backend_test_macros::database_test]
    async fn missing_token_header_returns_401(db: gateway_postgres::DatabaseHandle) {
        let (_, server) = create_test_server_with_state(db.clone()).await;

        let res = server
            .post("/api/agent-tasks/notifications")
            .json(&json!({"id": "task-1"}))
            .await;
        assert_eq!(
            (res.status_code(), reconciliation_jobs(&db).await),
            (StatusCode::UNAUTHORIZED, vec![]),
        );
    }
}

mod hooks {
    use std::sync::Arc;

    use axum::http::StatusCode;
    use core_application::agent_task_client::{
        AgentTaskError, FakeAgentTaskClient, SharedAgentTaskClient,
    };
    use core_application::strategy_task::DEFAULT_PURPOSE;
    use gateway_postgres::entities::sea_orm_active_enums::StrategyTaskPhase;
    use gateway_postgres::entities::strategy_task;
    use sea_orm::{EntityTrait, QueryOrder};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::testing::agent_config;
    use crate::testing::{
        create_test_server_with_db_and_agent_client, insert_test_hook_trigger, insert_test_strategy,
    };

    #[derive(Debug, PartialEq, Eq)]
    struct TaskShape {
        strategy_id: Uuid,
        source: String,
        prompt: String,
        phase: StrategyTaskPhase,
    }

    impl TaskShape {
        fn from(row: &strategy_task::Model) -> Self {
            Self {
                strategy_id: row.strategy_id,
                source: row.source.clone(),
                prompt: row.prompt.clone(),
                phase: row.phase.clone(),
            }
        }
    }

    #[backend_test_macros::database_test]
    async fn fires_when_event_match_satisfied(db: gateway_postgres::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
        let _ = insert_test_hook_trigger(
            &db,
            strategy_id,
            "sample-hook",
            "alert {{payload.symbol}} for {{strategy.name}}",
            Some(json!({"event": {"eq": "fired"}})),
            true,
        )
        .await;

        let res = server
            .post("/api/hooks/sample-hook")
            .json(&json!({"event": "fired", "symbol": "sample-symbol"}))
            .await;
        res.assert_status_ok();
        let mut body: Value = res.json();
        let task_id_str = body["task_id"].as_str().unwrap().to_string();
        let task_id = Uuid::parse_str(&task_id_str).unwrap();
        body["task_id"] = Value::String("<task_id>".to_string());
        let task = strategy_task::Entity::find_by_id(task_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (body, TaskShape::from(&task)),
            (
                json!({ "fired": true, "task_id": "<task_id>" }),
                TaskShape {
                    strategy_id,
                    source: "hook".to_string(),
                    prompt: "alert sample-symbol for sample-strategy".to_string(),
                    phase: StrategyTaskPhase::Running,
                },
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn skips_when_event_match_not_satisfied(db: gateway_postgres::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
        let _ = insert_test_hook_trigger(
            &db,
            strategy_id,
            "sample-hook",
            "sample prompt",
            Some(json!({"event": {"eq": "fired"}})),
            true,
        )
        .await;

        let res = server
            .post("/api/hooks/sample-hook")
            .json(&json!({"event": "ignored"}))
            .await;
        res.assert_status_ok();
        assert_eq!(res.json::<Value>(), json!({"fired": false}));

        let tasks = strategy_task::Entity::find()
            .order_by_asc(strategy_task::Column::CreatedAt)
            .all(&db)
            .await
            .unwrap();
        assert!(tasks.is_empty());
    }

    #[backend_test_macros::database_test]
    async fn disabled_trigger_is_404(db: gateway_postgres::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
        let _ = insert_test_hook_trigger(
            &db,
            strategy_id,
            "disabled-hook",
            "sample prompt",
            None,
            false,
        )
        .await;

        let res = server
            .post("/api/hooks/disabled-hook")
            .json(&json!({}))
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn unknown_slug_is_404(db: gateway_postgres::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (_db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let res = server
            .post("/api/hooks/unknown-hook")
            .json(&json!({}))
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn placeholders_expand_from_payload(db: gateway_postgres::DatabaseHandle) {
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
        let _ = insert_test_hook_trigger(
            &db,
            strategy_id,
            "sample-hook",
            "symbol={{payload.symbol}} price={{payload.price}}",
            None,
            true,
        )
        .await;

        let res = server
            .post("/api/hooks/sample-hook")
            .json(&json!({"symbol": "sample-symbol", "price": 2500}))
            .await;
        res.assert_status_ok();

        let tasks = strategy_task::Entity::find()
            .order_by_asc(strategy_task::Column::CreatedAt)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(
            tasks.iter().map(TaskShape::from).collect::<Vec<_>>(),
            vec![TaskShape {
                strategy_id,
                source: "hook".to_string(),
                prompt: "symbol=sample-symbol price=2500".to_string(),
                phase: StrategyTaskPhase::Running,
            }],
        );
    }

    #[backend_test_macros::database_test]
    async fn agent_not_configured_returns_503(db: gateway_postgres::DatabaseHandle) {
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_submit_error(AgentTaskError::NotConfigured).await;
        let agent_client: SharedAgentTaskClient = fake;
        let (db, server) = create_test_server_with_db_and_agent_client(db, agent_client).await;
        let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
        agent_config::create(&db, DEFAULT_PURPOSE.to_string())
            .await
            .expect("insert test agent_config");
        let _ =
            insert_test_hook_trigger(&db, strategy_id, "sample-hook", "sample prompt", None, true)
                .await;

        let res = server.post("/api/hooks/sample-hook").json(&json!({})).await;
        res.assert_status(StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            res.json::<Value>(),
            json!({ "error": "agent task client is not configured" }),
        );
    }
}
