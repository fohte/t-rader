use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub published_date: Option<String>,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearchResponse {
    pub results: Vec<WebSearchResult>,
    pub credits_used: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebPageExtractionResponse {
    pub raw_content: Option<String>,
    pub failure: Option<WebPageExtractionFailure>,
    pub credits_used: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebPageExtractionFailure {
    pub url: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearchAccountUsage {
    pub current_plan: String,
    pub plan_credits_used: Decimal,
    pub paygo_credits_used: Decimal,
}

impl WebSearchAccountUsage {
    pub fn total_credits_used(&self) -> Option<Decimal> {
        self.plan_credits_used.checked_add(self.paygo_credits_used)
    }
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebSearchError {
    #[error("network error: {0}")]
    Network(String),

    #[error("network request timed out")]
    NetworkTimeout,

    #[error("web search API error (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("failed to parse web search response: {0}")]
    Parse(String),

    #[error("failed to extract page {url}: {message}")]
    ExtractionFailed { url: String, message: String },

    #[error("web search client initialization error: {0}")]
    Init(String),

    #[error("web search account usage could not be confirmed: {0}")]
    Usage(String),

    #[error("web search account usage request timed out")]
    UsageTimeout,
}

impl WebSearchError {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Network(_) => "network",
            Self::NetworkTimeout => "network_timeout",
            Self::Api { .. } => "provider_api",
            Self::Parse(_) => "response_parse",
            Self::ExtractionFailed { .. } => "extraction_failure",
            Self::Init(_) => "initialization",
            Self::Usage(_) => "usage_confirmation",
            Self::UsageTimeout => "usage_timeout",
        }
    }
}

#[async_trait]
pub trait WebSearchClient: Send + Sync {
    async fn search(
        &self,
        query: &str,
        topic: Option<WebSearchTopic>,
        time_range: Option<WebSearchTimeRange>,
    ) -> Result<WebSearchResponse, WebSearchError>;

    async fn extract_page(&self, url: &str) -> Result<WebPageExtractionResponse, WebSearchError>;

    async fn account_usage(&self) -> Result<WebSearchAccountUsage, WebSearchError>;
}

pub type SharedWebSearchClient = Arc<dyn WebSearchClient + Send + Sync>;
