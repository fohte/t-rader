use core_application::custom_indicator::CustomIndicator;
use sea_orm::entity::prelude::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = CustomIndicator)]
pub struct CustomIndicatorResponse {
    pub indicator_id: Uuid,
    pub name: String,
    pub scope: String,
    pub strategy_id: Option<Uuid>,
    pub code: String,
    pub input_schema: Json,
    pub output_schema: Json,
    pub description: Option<String>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
}

impl From<CustomIndicator> for CustomIndicatorResponse {
    fn from(model: CustomIndicator) -> Self {
        Self {
            indicator_id: model.indicator_id,
            name: model.name,
            scope: model.scope,
            strategy_id: model.strategy_id,
            code: model.code,
            input_schema: model.input_schema,
            output_schema: model.output_schema,
            description: model.description,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateCustomIndicatorRequest {
    #[schema(min_length = 1, pattern = r"\S")]
    pub name: String,
    pub code: String,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub input_schema: serde_json::Value,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub output_schema: serde_json::Value,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateCustomIndicatorRequest {
    #[schema(min_length = 1, pattern = r"\S")]
    pub name: Option<String>,
    pub code: Option<String>,
    #[schema(value_type = Option<std::collections::HashMap<String, serde_json::Value>>)]
    pub input_schema: Option<serde_json::Value>,
    #[schema(value_type = Option<std::collections::HashMap<String, serde_json::Value>>)]
    pub output_schema: Option<serde_json::Value>,
    // 未指定 (= 変更しない) と null 指定 (= clear) を区別するため double Option を使う
    #[serde(
        default,
        deserialize_with = "crate::serde_helpers::deserialize_nullable_option"
    )]
    #[schema(value_type = Option<String>)]
    pub description: Option<Option<String>>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewIndicatorRequest {
    pub code: String,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub input_schema: serde_json::Value,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub output_schema: serde_json::Value,
    #[schema(value_type = serde_json::Value)]
    pub args: serde_json::Value,
    #[serde(default)]
    pub timeout_secs: Option<u32>,
    #[serde(default)]
    pub max_output_bytes: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PreviewIndicatorResponse {
    /// stdout 最終行を JSON parse し output_schema で validation 済みの値。
    /// exec Pod が exit_code != 0 で終わった場合は null (stderr / exit_code を参照)。
    #[schema(value_type = Option<serde_json::Value>)]
    pub output: Option<serde_json::Value>,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}
