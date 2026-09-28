use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use sea_orm::entity::prelude::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use gateway_postgres::entities::agent_config;

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
    /// 多段フェーズ実行設定 (YAML)。未設定なら空文字列。
    pub agent_graph: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = AgentConfig)]
pub struct AgentConfigItemResponse {
    pub id: Uuid,
    pub purpose: String,
    pub agents_md: String,
    pub skills: Json,
    pub agent_graph: String,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub updated_at: DateTime<FixedOffset>,
}

impl From<agent_config::Model> for AgentConfigItemResponse {
    fn from(model: agent_config::Model) -> Self {
        Self {
            id: model.id,
            purpose: model.purpose,
            agents_md: model.agents_md,
            skills: model.skills,
            agent_graph: model.agent_graph,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}
