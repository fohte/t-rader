//! t-rader-agent からのタスク決着通知 (push notification) を受信する endpoint。
//!
//! body は A2A Task オブジェクト全体だが内容は信用しない。正データは watcher が
//! 内部 API (`GET /internal/tasks/:id`) から取得するため、ここでは認証を検証した上で
//! watcher の即時 polling を誘発するだけに留める (polling が決着の正経路であり、
//! この通知は即時性のための最適化に過ぎない)。

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
        (status = 204, description = "受理 (watcher の即時 polling を誘発)"),
        (status = 401, description = "トークン不一致", body = ErrorResponse),
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
        return Err(AppError::Unauthorized("invalid notification token".into()));
    }
    state.agent_task_notify.notify_one();
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;
    use sqlx::PgPool;

    use crate::testing::create_test_server_with_state;

    use super::*;

    #[sqlx::test(migrations = false)]
    async fn valid_token_returns_204_and_notifies_watcher(pool: PgPool) {
        let (state, server) = create_test_server_with_state(pool).await;
        let notified = state.agent_task_notify.notified();

        let res = server
            .post("/api/agent-tasks/notifications")
            .add_header(
                NOTIFICATION_TOKEN_HEADER,
                state.agent_webhook_token.as_ref(),
            )
            .json(&json!({"id": "task-1", "status": {"state": "completed"}}))
            .await;
        res.assert_status(axum::http::StatusCode::NO_CONTENT);

        tokio::time::timeout(Duration::from_millis(200), notified)
            .await
            .expect("watcher should have been notified");
    }

    #[sqlx::test(migrations = false)]
    async fn mismatched_token_returns_401(pool: PgPool) {
        let (_state, server) = create_test_server_with_state(pool).await;

        let res = server
            .post("/api/agent-tasks/notifications")
            .add_header(NOTIFICATION_TOKEN_HEADER, "wrong-token")
            .json(&json!({"id": "task-1"}))
            .await;
        res.assert_status(axum::http::StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test(migrations = false)]
    async fn missing_token_header_returns_401(pool: PgPool) {
        let (_state, server) = create_test_server_with_state(pool).await;

        let res = server
            .post("/api/agent-tasks/notifications")
            .json(&json!({"id": "task-1"}))
            .await;
        res.assert_status(axum::http::StatusCode::UNAUTHORIZED);
    }
}
