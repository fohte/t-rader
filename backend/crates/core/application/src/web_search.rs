use std::sync::Arc;

use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSearchTopic {
    General,
    News,
}

impl WebSearchTopic {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::News => "news",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSearchTimeRange {
    Day,
    Week,
    Month,
    Year,
}

impl WebSearchTimeRange {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub published_date: Option<String>,
    pub snippet: String,
    pub body: Option<String>,
    pub body_truncated: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum WebSearchError {
    #[error("network error: {0}")]
    Network(String),

    #[error("web search API error (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("failed to parse web search response: {0}")]
    Parse(String),

    #[error("web search client initialization error: {0}")]
    Init(String),
}

#[async_trait]
pub trait WebSearchClient: Send + Sync {
    async fn search(
        &self,
        query: &str,
        topic: Option<WebSearchTopic>,
        time_range: Option<WebSearchTimeRange>,
    ) -> Result<Vec<WebSearchResult>, WebSearchError>;
}

pub type SharedWebSearchClient = Arc<dyn WebSearchClient + Send + Sync>;
