use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use axum::http::StatusCode;
use core_application::agent_task_client::{
    AgentTaskClient, AgentTaskError, AgentTaskRef, AgentTaskState, AgentTaskStatus,
    SharedAgentTaskClient, SubmitAgentTask,
};
use core_application::{
    persistence::PersistenceError,
    strategy_task::{
        StrategyTaskReconcileLock, StrategyTaskReconcileLockGuard,
        StrategyTaskReconcileLockUseCases,
    },
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

struct ReconcileLockState {
    locked: tokio::sync::Mutex<bool>,
    acquire_started: Notify,
    released: Notify,
}

#[derive(Clone)]
struct BlockingReconcileLock(Arc<ReconcileLockState>);

struct BlockingReconcileLockGuard(Arc<ReconcileLockState>);

struct PanicOnceReconcileLock(AtomicUsize);

struct EmptyReconcileLockGuard;

impl BlockingReconcileLock {
    fn new() -> Self {
        Self(Arc::new(ReconcileLockState {
            locked: tokio::sync::Mutex::new(false),
            acquire_started: Notify::new(),
            released: Notify::new(),
        }))
    }
}

#[async_trait]
impl StrategyTaskReconcileLock for BlockingReconcileLock {
    async fn acquire(&self) -> Result<Box<dyn StrategyTaskReconcileLockGuard>, PersistenceError> {
        self.0.acquire_started.notify_one();
        loop {
            let released = self.0.released.notified();
            if let Some(guard) = self.try_acquire().await? {
                return Ok(guard);
            }
            released.await;
        }
    }

    async fn try_acquire(
        &self,
    ) -> Result<Option<Box<dyn StrategyTaskReconcileLockGuard>>, PersistenceError> {
        let mut locked = self.0.locked.lock().await;
        if *locked {
            return Ok(None);
        }
        *locked = true;
        Ok(Some(Box::new(BlockingReconcileLockGuard(self.0.clone()))))
    }
}

#[async_trait]
impl StrategyTaskReconcileLock for PanicOnceReconcileLock {
    async fn acquire(&self) -> Result<Box<dyn StrategyTaskReconcileLockGuard>, PersistenceError> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            std::panic::panic_any("synthetic lock acquisition panic");
        }
        Ok(Box::new(EmptyReconcileLockGuard))
    }

    async fn try_acquire(
        &self,
    ) -> Result<Option<Box<dyn StrategyTaskReconcileLockGuard>>, PersistenceError> {
        Ok(Some(Box::new(EmptyReconcileLockGuard)))
    }
}

#[async_trait]
impl StrategyTaskReconcileLockGuard for BlockingReconcileLockGuard {
    async fn release(self: Box<Self>) -> Result<(), PersistenceError> {
        *self.0.locked.lock().await = false;
        self.0.released.notify_one();
        Ok(())
    }
}

#[async_trait]
impl StrategyTaskReconcileLockGuard for EmptyReconcileLockGuard {
    async fn release(self: Box<Self>) -> Result<(), PersistenceError> {
        Ok(())
    }
}

struct ReconcileAfterNotificationAgentTaskClient {
    get_calls: AtomicUsize,
    first_get_started: Notify,
    resume_first_get: Notify,
    second_get_started: Notify,
}

impl ReconcileAfterNotificationAgentTaskClient {
    fn new() -> Self {
        Self {
            get_calls: AtomicUsize::new(0),
            first_get_started: Notify::new(),
            resume_first_get: Notify::new(),
            second_get_started: Notify::new(),
        }
    }
}

#[async_trait]
impl AgentTaskClient for ReconcileAfterNotificationAgentTaskClient {
    async fn submit(&self, _request: SubmitAgentTask) -> Result<AgentTaskRef, AgentTaskError> {
        Err(AgentTaskError::NotConfigured)
    }

    async fn get(&self, _task_id: &str) -> Result<AgentTaskStatus, AgentTaskError> {
        let get_call = self.get_calls.fetch_add(1, Ordering::SeqCst);
        let state = if get_call == 0 {
            self.first_get_started.notify_one();
            self.resume_first_get.notified().await;
            AgentTaskState::Working
        } else {
            self.second_get_started.notify_one();
            AgentTaskState::Completed
        };
        Ok(AgentTaskStatus {
            state,
            result_text: Some("synthetic result".to_string()),
            error_message: None,
            error_kind: None,
            steps: None,
        })
    }
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
async fn notification_reconciles_again_after_an_existing_run_finishes(
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

    let fake = Arc::new(ReconcileAfterNotificationAgentTaskClient::new());
    let agent_client: SharedAgentTaskClient = fake.clone();
    let lock = BlockingReconcileLock::new();
    let lock_use_cases = StrategyTaskReconcileLockUseCases::new(Arc::new(lock.clone()));
    let use_cases = crate::services::use_cases::build_use_cases(db.clone(), agent_client.clone());
    let reconcile = use_cases.strategy_task_reconcile(lock_use_cases.clone());
    let first_reconcile = tokio::spawn(async move { reconcile.reconcile().await });
    tokio::time::timeout(Duration::from_secs(2), fake.first_get_started.notified())
        .await
        .expect("first reconciliation should load the task list");

    let server = crate::testing::create_test_server_with_db_agent_client_and_reconcile_lock(
        db.clone(),
        agent_client,
        lock_use_cases,
    )
    .await;
    let response = tokio::time::timeout(
        Duration::from_secs(2),
        server
            .post("/api/agent-tasks/notifications")
            .add_header(NOTIFICATION_TOKEN_HEADER, TEST_AGENT_WEBHOOK_TOKEN)
            .json(&serde_json::json!({"id": "synthetic-agent-task"})),
    )
    .await
    .expect("webhook should respond while waiting for the reconcile lock");
    let webhook_waited_for_lock =
        tokio::time::timeout(Duration::from_secs(2), lock.0.acquire_started.notified())
            .await
            .is_ok();

    fake.resume_first_get.notify_one();
    let first_reconcile_result = tokio::time::timeout(Duration::from_secs(2), first_reconcile)
        .await
        .expect("first reconciliation should finish")
        .expect("first reconciliation task should not panic");
    tokio::time::timeout(Duration::from_secs(2), fake.second_get_started.notified())
        .await
        .expect("webhook reconciliation should fetch the task again");
    let reconciled_phase = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let task = strategy_task::Entity::find_by_id(task_id)
                .one(&db)
                .await
                .expect("read reconciled strategy task")
                .expect("strategy task exists");
            if task.phase == StrategyTaskPhase::Completed {
                break task.phase;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("webhook reconciliation should apply the completed state");

    assert_eq!(
        (
            response.status_code(),
            webhook_waited_for_lock,
            first_reconcile_result,
            reconciled_phase,
        ),
        (
            StatusCode::NO_CONTENT,
            true,
            Ok(1),
            StrategyTaskPhase::Completed,
        ),
    );
}

#[backend_test_macros::database_test]
async fn waiting_reconcile_releases_its_gate_after_lock_acquisition_panics(
    db: gateway_postgres::DatabaseHandle,
) {
    let lock = Arc::new(PanicOnceReconcileLock(AtomicUsize::new(0)));
    let lock_use_cases = StrategyTaskReconcileLockUseCases::new(lock.clone());
    let use_cases = crate::services::use_cases::build_use_cases(
        db,
        Arc::new(ReconcileAfterNotificationAgentTaskClient::new()),
    );
    let reconcile = use_cases.strategy_task_reconcile(lock_use_cases);

    let first = reconcile.reconcile_waiting_for_lock().await;
    let second = reconcile.reconcile_waiting_for_lock().await;

    assert_eq!(
        (first, second, lock.0.load(Ordering::SeqCst),),
        (
            Err("strategy task reconciliation panicked".to_string()),
            Ok(0),
            2,
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
