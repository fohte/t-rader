use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewsContentFetchOutcome {
    Fetched(String),
    Failed(String),
    Retry,
    Abort(NewsContentInterruption),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NewsContentInterruption {
    FirecrawlCreditsExhausted,
    FirecrawlRateLimited,
}

impl NewsContentInterruption {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FirecrawlCreditsExhausted => "firecrawl_402",
            Self::FirecrawlRateLimited => "firecrawl_429",
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NewsContentFetchError {
    #[error("rate limiter is unavailable")]
    RateLimiter,
}

#[async_trait]
pub trait NewsContentFetcher: Send + Sync {
    async fn fetch(&self, url: &str) -> Result<NewsContentFetchOutcome, NewsContentFetchError>;
}

pub type SharedNewsContentFetcher = std::sync::Arc<dyn NewsContentFetcher + Send + Sync>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct NewsContentFetchStats {
    pub expired: u64,
    pub fetched: u64,
    pub failed: u64,
    pub retried: u64,
    pub interrupted_reason: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{message}")]
pub struct NewsContentRunError {
    pub stats: NewsContentFetchStats,
    pub message: String,
}
