use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateAgentConfigRequest {
    /// 目的キー (slug, `^[a-z0-9][a-z0-9_-]*$`)。セマンティックな分類はコードに持たず、
    /// この値自体が呼び出し側の決めた自由記述の目的名になる。
    #[schema(min_length = 1, pattern = r"^[a-z0-9][a-z0-9_-]*$")]
    pub purpose: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentsMdBody {
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillBody {
    pub content: String,
}

/// 目的ごとの多段フェーズ実行設定 (YAML)。未設定の場合は `content` が空文字列。
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentGraphBody {
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillsBody {
    pub skills: BTreeMap<String, String>,
}

/// t-rader-agent がタスク実行時に取得する agent 設定一式。
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentConfigResponse {
    pub agents_md: String,
    pub skills: BTreeMap<String, String>,
    pub model: String,
    pub small_model: String,
    /// 多段フェーズ実行設定 (YAML)。未設定なら空文字列。
    pub agent_graph: String,
}
