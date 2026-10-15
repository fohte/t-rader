use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::http::StatusCode;
use core_application::agent_task_client::{
    AgentTaskClient, AgentTaskError, AgentTaskRef, AgentTaskState, AgentTaskStatus,
    SharedAgentTaskClient, SubmitAgentTask,
};
use gateway_postgres::entities::sea_orm_active_enums::StrategyTaskPhase;
use gateway_postgres::entities::{event, strategy_task};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, IntoActiveModel};
use tokio::sync::Notify;

use crate::testing::{
    TEST_AGENT_WEBHOOK_TOKEN, create_test_server, create_test_server_with_db_and_agent_client,
    insert_test_strategy, insert_test_strategy_task,
};

const NOTIFICATION_TOKEN_HEADER: &str = "x-a2a-notification-token";

struct BlockingGetAgentTaskClient {
    get_started: Notify,
    resume_get: Notify,
}

impl BlockingGetAgentTaskClient {
    fn new() -> Self {
        Self {
            get_started: Notify::new(),
            resume_get: Notify::new(),
        }
    }
}

#[async_trait]
impl AgentTaskClient for BlockingGetAgentTaskClient {
    async fn submit(&self, _request: SubmitAgentTask) -> Result<AgentTaskRef, AgentTaskError> {
        Err(AgentTaskError::NotConfigured)
    }

    async fn get(&self, _task_id: &str) -> Result<AgentTaskStatus, AgentTaskError> {
        self.get_started.notify_one();
        self.resume_get.notified().await;
        Ok(AgentTaskStatus {
            state: AgentTaskState::Completed,
            result_text: Some("synthetic result".to_string()),
            error_message: None,
            error_kind: None,
            steps: None,
        })
    }
}

#[backend_test_macros::database_test]
async fn valid_token_returns_204_before_background_reconciliation_finishes(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_test_strategy(&db, "synthetic strategy").await;
    let task_id = insert_test_strategy_task(
        &db,
        strategy_id,
        "synthetic prompt",
        None,
        chrono::Utc::now().fixed_offset(),
    )
    .await;
    let mut task = strategy_task::Entity::find_by_id(task_id)
        .one(&db)
        .await
        .expect("read seeded strategy task")
        .expect("strategy task exists")
        .into_active_model();
    task.a2a_task_id = Set(Some("synthetic-agent-task".to_string()));
    task.phase = Set(StrategyTaskPhase::Running);
    task.result_text = Set(None);
    task.update(&db)
        .await
        .expect("mark strategy task in flight");

    let fake = Arc::new(BlockingGetAgentTaskClient::new());
    let agent_client: SharedAgentTaskClient = fake.clone();
    let (_, server) = create_test_server_with_db_and_agent_client(db.clone(), agent_client).await;
    let response = tokio::time::timeout(
        Duration::from_secs(2),
        server
            .post("/api/agent-tasks/notifications")
            .add_header(NOTIFICATION_TOKEN_HEADER, TEST_AGENT_WEBHOOK_TOKEN)
            .json(&serde_json::json!({"id": "synthetic-agent-task"})),
    )
    .await
    .expect("webhook should respond while reconciliation is blocked");
    tokio::time::timeout(Duration::from_secs(2), fake.get_started.notified())
        .await
        .expect("background reconciliation should query the task");

    fake.resume_get.notify_one();
    let reconciled = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let task = strategy_task::Entity::find_by_id(task_id)
                .one(&db)
                .await
                .expect("read reconciled strategy task")
                .expect("strategy task exists");
            let completion_event_dispatched = event::Entity::find()
                .all(&db)
                .await
                .expect("read strategy task events")
                .into_iter()
                .any(|event| {
                    event.event_type == "strategy_task.completed"
                        && event.payload["task_id"] == task_id.to_string()
                        && event.dispatched_at.is_some()
                });
            if task.phase == StrategyTaskPhase::Completed && completion_event_dispatched {
                break (task.phase, task.result_text);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("background reconciliation should finish");

    assert_eq!(
        (response.status_code(), reconciled),
        (
            StatusCode::NO_CONTENT,
            (
                StrategyTaskPhase::Completed,
                Some("synthetic result".to_string()),
            ),
        ),
    );
}

#[backend_test_macros::database_test]
async fn mismatched_token_returns_401(db: gateway_postgres::DatabaseHandle) {
    let server = create_test_server(db).await;

    let res = server
        .post("/api/agent-tasks/notifications")
        .add_header(NOTIFICATION_TOKEN_HEADER, "wrong-token")
        .json(&serde_json::json!({"id": "synthetic-agent-task"}))
        .await;

    assert_eq!(
        (res.status_code(), res.json::<serde_json::Value>()),
        (
            StatusCode::UNAUTHORIZED,
            serde_json::json!({ "error": "invalid notification token" }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn missing_token_header_returns_401(db: gateway_postgres::DatabaseHandle) {
    let server = create_test_server(db).await;

    let res = server
        .post("/api/agent-tasks/notifications")
        .json(&serde_json::json!({"id": "synthetic-agent-task"}))
        .await;

    assert_eq!(
        (res.status_code(), res.json::<serde_json::Value>()),
        (
            StatusCode::UNAUTHORIZED,
            serde_json::json!({ "error": "invalid notification token" }),
        ),
    );
}
