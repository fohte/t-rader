use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use core_application::rss_feed::RssFeed;

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
    /// 本文取得元。none は本文なし、feed は RSS 本文、crawl はリンク先取得。省略時は none
    #[serde(default = "default_content_source")]
    pub content_source: String,
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
    /// 本文取得元。none / feed / crawl のいずれか。省略時は現在の設定を維持する。
    #[serde(default)]
    pub content_source: Option<String>,
}

fn default_content_source() -> String {
    "none".to_owned()
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
    /// 本文の取得方式。none / feed / crawl のいずれか。
    pub content_source: String,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub updated_at: DateTime<FixedOffset>,
}

impl From<RssFeed> for RssFeedResponse {
    fn from(model: RssFeed) -> Self {
        Self {
            id: model.id,
            source: model.source,
            display_name: model.display_name,
            url: model.url,
            enabled: model.enabled,
            content_source: model.content_source,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}
