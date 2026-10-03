use chrono::{DateTime, FixedOffset};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RssFeed {
    pub id: Uuid,
    pub source: String,
    pub display_name: String,
    pub url: String,
    pub enabled: bool,
    pub content_source: String,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRssFeedCommand {
    pub source: String,
    pub display_name: String,
    pub url: String,
    pub enabled: Option<bool>,
    pub content_source: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateRssFeedPatch {
    pub display_name: Option<String>,
    pub url: Option<String>,
    pub enabled: Option<bool>,
    pub content_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRssFeed {
    pub id: Uuid,
    pub source: String,
    pub display_name: String,
    pub url: String,
    pub enabled: bool,
    pub content_source: String,
}
