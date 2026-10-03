mod tests_common;

use axum::http::StatusCode;
use core_application::strategy_task::STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER;
use serde_json::{Value, json};

use self::tests_common::{
    NOTIFICATION_TOKEN_HEADER, TEST_AGENT_WEBHOOK_TOKEN, reconciliation_jobs,
};
use crate::testing::create_test_server_with_graphile_worker;

#[backend_test_macros::database_test]
async fn valid_token_returns_204_and_enqueues_reconciliation(db: gateway_postgres::DatabaseHandle) {
    let server = create_test_server_with_graphile_worker(db.clone()).await;

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
    let server = create_test_server_with_graphile_worker(db.clone()).await;

    let res = server
        .post("/api/agent-tasks/notifications")
        .add_header(NOTIFICATION_TOKEN_HEADER, "wrong-token")
        .json(&json!({"id": "task-1"}))
        .await;
    assert_eq!(
        (
            res.status_code(),
            res.json::<Value>(),
            reconciliation_jobs(&db).await,
        ),
        (
            StatusCode::UNAUTHORIZED,
            json!({ "error": "invalid notification token" }),
            vec![],
        ),
    );
}

#[backend_test_macros::database_test]
async fn missing_token_header_returns_401(db: gateway_postgres::DatabaseHandle) {
    let server = create_test_server_with_graphile_worker(db.clone()).await;

    let res = server
        .post("/api/agent-tasks/notifications")
        .json(&json!({"id": "task-1"}))
        .await;
    assert_eq!(
        (
            res.status_code(),
            res.json::<Value>(),
            reconciliation_jobs(&db).await,
        ),
        (
            StatusCode::UNAUTHORIZED,
            json!({ "error": "invalid notification token" }),
            vec![],
        ),
    );
}
