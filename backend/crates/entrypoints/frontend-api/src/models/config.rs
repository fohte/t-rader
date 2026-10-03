use serde::Serialize;
use utoipa::ToSchema;

/// `GET /api/config` の戻り値。frontend に渡す軽量なランタイム設定値。
#[derive(Debug, PartialEq, Serialize, ToSchema)]
pub struct ConfigResponse {
    /// トレースビューアの URL テンプレート (`{trace_id}`/`{span_id}` プレースホルダを含む)。
    /// `TRACE_URL_TEMPLATE` 未設定なら `null`。
    pub trace_url_template: Option<String>,
}
