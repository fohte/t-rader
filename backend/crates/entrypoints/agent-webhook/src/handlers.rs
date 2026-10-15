use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonBody;
use crate::state::AgentWebhookState;

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
        (status = 204, description = "受理 (戦略タスクの状態同期をバックグラウンドで開始)"),
        (status = 401, description = "トークン不一致", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
    )
)]
async fn receive_agent_task_notification(
    State(state): State<AgentWebhookState>,
    headers: HeaderMap,
    JsonBody(_payload): JsonBody<serde_json::Value>,
) -> Result<StatusCode, AppError> {
    let presented = headers
        .get(NOTIFICATION_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if !constant_time_eq(presented.as_bytes(), state.webhook_token.as_bytes()) {
        tracing::warn!("agent task notification rejected: token mismatch");
        return Err(AppError::Unauthorized("invalid notification token".into()));
    }
    let reconcile_use_cases = state.strategy_task_reconcile_use_cases.clone();
    tokio::spawn(async move {
        if let Err(error) = reconcile_use_cases.reconcile().await {
            tracing::warn!(%error, "agent task notification reconciliation failed");
        }
    });
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> OpenApiRouter<AgentWebhookState> {
    OpenApiRouter::new().routes(routes!(receive_agent_task_notification))
}
