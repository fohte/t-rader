use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::entities::rss_feed;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRssFeedRequest {
    /// machine key (slug, `^[a-z0-9_-]+$`). 内部処理・MCP の参照用
    pub source: String,
    /// UI 表示用名前
    pub display_name: String,
    /// RSS フィード URL (http / https のみ)
    pub url: String,
    /// 省略時は true
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateRssFeedRequest {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct ListRssFeedsQuery {
    /// true なら enabled=true のフィードだけ返す
    #[serde(default)]
    pub enabled_only: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = RssFeed)]
pub struct RssFeedResponse {
    pub id: Uuid,
    pub source: String,
    pub display_name: String,
    pub url: String,
    pub enabled: bool,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub updated_at: DateTime<FixedOffset>,
}

impl From<rss_feed::Model> for RssFeedResponse {
    fn from(model: rss_feed::Model) -> Self {
        Self {
            id: model.id,
            source: model.source,
            display_name: model.display_name,
            url: model.url,
            enabled: model.enabled,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}
