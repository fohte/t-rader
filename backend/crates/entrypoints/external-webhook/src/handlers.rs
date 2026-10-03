use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::{AppError, ErrorResponse, map_hook_error};
use crate::extractors::{JsonBody, JsonPath};
use crate::state::ExternalWebhookState;

/// hook 受信レスポンス。
///
/// `fired = true`: trigger に紐づく strategy_task が作成された。
/// `fired = false`: payload が `event_match` を満たさなかったため no-op (200 OK)。
#[derive(Debug, Serialize, ToSchema)]
pub struct HookResponse {
    /// 発火したか
    pub fired: bool,
    /// 発火時のみ。作成された strategy_task の UUID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<uuid::Uuid>,
}

/// hook を受信する。
///
/// - `hook_slug` に一致する有効な trigger が無ければ 404
/// - trigger が `enabled=false` の場合も 404 (外部に存在を漏らさない)
/// - `event_match` を満たさない payload は 200 OK の no-op
/// - 満たした場合は共通 service 経由で `submit_strategy_task` を呼び、200 OK を返す
#[utoipa::path(
    post,
    path = "/api/hooks/{hook_slug}",
    tag = "triggers",
    params(("hook_slug" = String, Path, description = "hook 識別子")),
    request_body = serde_json::Value,
    responses(
        (status = 200, body = HookResponse),
        (status = 400, description = "リクエストボディに null バイトが含まれる等の汎用エラー", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
        (status = 503, description = "agent task client が未設定、または agent_config が見つからない", body = ErrorResponse),
    )
)]
async fn receive_hook(
    State(state): State<ExternalWebhookState>,
    JsonPath(hook_slug): JsonPath<String>,
    JsonBody(payload): JsonBody<serde_json::Value>,
) -> Result<(StatusCode, Json<HookResponse>), AppError> {
    match state
        .trigger_use_cases
        .fire_hook(state.agent_task_client.as_ref(), &hook_slug, payload)
        .await
    {
        Ok(Some(outcome)) => Ok((
            StatusCode::OK,
            Json(HookResponse {
                fired: true,
                task_id: Some(outcome.task_id),
            }),
        )),
        Ok(None) => Ok((
            StatusCode::OK,
            Json(HookResponse {
                fired: false,
                task_id: None,
            }),
        )),
        Err(error) => Err(map_hook_error(&hook_slug, error)),
    }
}

pub fn router() -> OpenApiRouter<ExternalWebhookState> {
    OpenApiRouter::new().routes(routes!(receive_hook))
}
