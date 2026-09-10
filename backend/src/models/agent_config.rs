use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateAgentConfigRequest {
    /// 目的キー (slug, `^[a-z0-9][a-z0-9_-]*$`)。セマンティックな分類はコードに持たず、
    /// この値自体が呼び出し側の決めた自由記述の目的名になる。
    #[schema(min_length = 1, pattern = r"^[a-z0-9][a-z0-9_-]*$")]
    pub purpose: String,
}
