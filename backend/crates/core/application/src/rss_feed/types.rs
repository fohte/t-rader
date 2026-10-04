use chrono::{DateTime, FixedOffset};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentSource {
    None,
    Feed,
    Crawl,
}

impl ContentSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Feed => "feed",
            Self::Crawl => "crawl",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "feed" => Some(Self::Feed),
            "crawl" => Some(Self::Crawl),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RssFeed {
    pub id: Uuid,
    pub source: String,
    pub display_name: String,
    pub url: String,
    pub enabled: bool,
    pub content_source: ContentSource,
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
pub struct UpdateRssFeedCommand {
    pub display_name: Option<String>,
    pub url: Option<String>,
    pub enabled: Option<bool>,
    pub content_source: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateRssFeedPatch {
    pub display_name: Option<String>,
    pub url: Option<String>,
    pub enabled: Option<bool>,
    pub content_source: Option<ContentSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRssFeed {
    pub id: Uuid,
    pub source: String,
    pub display_name: String,
    pub url: String,
    pub enabled: bool,
    pub content_source: ContentSource,
}
