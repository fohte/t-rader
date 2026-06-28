use serde::Deserialize;
use utoipa::ToSchema;

/// 戦略の関心 (interest) を新規追加するリクエスト。
///
/// `ref_kind` は参照型 (`stock` / `indicator` / `sector` / `theme`) を指定する。
/// `role` は省略時 `seed`、`origin` は省略時 `human`。
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateInterestRequest {
    pub ref_kind: String,
    pub ref_id: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub origin: Option<String>,
}

/// 既存の関心の role / origin を更新するリクエスト。
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateInterestRequest {
    pub role: Option<String>,
    pub origin: Option<String>,
}
