use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;

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
    #[serde(default, deserialize_with = "deserialize_some")]
    #[schema(value_type = Option<String>)]
    pub description: Option<Option<String>>,
}

fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
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
