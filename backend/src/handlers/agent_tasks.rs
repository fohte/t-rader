//! t-rader-agent からのタスク決着通知 (push notification) を受信する endpoint。
//!
//! body は A2A Task オブジェクト全体だが内容は信用しない。正データは scheduler job が
//! 内部 API (`GET /internal/tasks/:id`) から取得するため、認証後に照合 job を投入する。

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonBody;

const NOTIFICATION_TOKEN_HEADER: &str = "x-a2a-notification-token";

/// 定数時間で 2 つのバイト列を比較する。長さの不一致は即座に false を返すが、
/// トークン自体の長さは秘匿情報ではないため許容する。
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// t-rader-agent からの push notification を受信する。
#[utoipa::path(
    post,
    path = "/api/agent-tasks/notifications",
    tag = "strategies",
    request_body = serde_json::Value,
    responses(
        (status = 204, description = "受理 (戦略タスク照合 job を投入)"),
        (status = 401, description = "トークン不一致", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
    )
)]
pub async fn receive_agent_task_notification(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonBody(_payload): JsonBody<serde_json::Value>,
) -> Result<StatusCode, AppError> {
    let presented = headers
        .get(NOTIFICATION_TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if !constant_time_eq(presented.as_bytes(), state.agent_webhook_token.as_bytes()) {
        // トークン誤設定は定期照合の裏に隠れて気づきにくいため、運用者向けに記録する。
        tracing::warn!("agent task notification rejected: token mismatch");
        return Err(AppError::Unauthorized("invalid notification token".into()));
    }
    state
        .use_cases
        .strategy_task_reconcile_job()
        .enqueue_reconciliation()
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use crate::testing::create_test_server_with_state;
    use core_application::strategy_task::STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER;
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
    use serde_json::json;

    use super::*;

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
        let (state, server) = create_test_server_with_state(db.clone()).await;

        let res = server
            .post("/api/agent-tasks/notifications")
            .add_header(
                NOTIFICATION_TOKEN_HEADER,
                state.agent_webhook_token.as_ref(),
            )
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
        let (_state, server) = create_test_server_with_state(db.clone()).await;

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
        let (_state, server) = create_test_server_with_state(db.clone()).await;

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
