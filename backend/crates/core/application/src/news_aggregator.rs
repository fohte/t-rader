use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NewsAggregatorError {
    #[error("client initialization error: {0}")]
    Initialization(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("api error (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("failed to parse response: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsFeed {
    pub source: String,
    pub url: String,
}

/// RSS aggregator が返す 1 件のニュース
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsItem {
    /// ソース表示名
    pub source: String,
    pub url: String,
    pub title: String,
    /// description などからの抜粋 (本文先頭 280 文字程度)
    pub body_snippet: Option<String>,
    pub published_at: DateTime<Utc>,
}

/// 指定された RSS フィードを集約して `NewsItem` 列を返す port
#[async_trait]
pub trait NewsAggregator: Send + Sync {
    async fn fetch_news(&self, feeds: &[NewsFeed]) -> Result<Vec<NewsItem>, NewsAggregatorError>;
}

pub type SharedNewsAggregator = Arc<dyn NewsAggregator>;

#[cfg(feature = "test-support")]
#[derive(Default)]
pub struct FakeNewsAggregator {
    pub requested_feeds: tokio::sync::Mutex<Vec<Vec<NewsFeed>>>,
    pub items: tokio::sync::Mutex<Vec<NewsItem>>,
    pub fetch_error: tokio::sync::Mutex<Option<NewsAggregatorError>>,
}

#[cfg(feature = "test-support")]
impl FakeNewsAggregator {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "test-support")]
#[async_trait]
impl NewsAggregator for FakeNewsAggregator {
    async fn fetch_news(&self, feeds: &[NewsFeed]) -> Result<Vec<NewsItem>, NewsAggregatorError> {
        self.requested_feeds.lock().await.push(feeds.to_vec());
        if let Some(err) = self.fetch_error.lock().await.take() {
            return Err(err);
        }
        Ok(self.items.lock().await.clone())
    }
}
