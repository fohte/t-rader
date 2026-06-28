pub mod rss;

use chrono::{DateTime, Utc};

use crate::data_provider::DataProviderError;

/// RSS aggregator が返す 1 件のニュース
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsItem {
    /// ソース表示名 (例: "Yahoo! Japan", "Bloomberg JP", "Reuters JP")
    pub source: String,
    pub url: String,
    pub title: String,
    /// description などからの抜粋 (本文先頭 280 文字程度)
    pub body_snippet: Option<String>,
    pub published_at: DateTime<Utc>,
}

/// 各 RSS フィードを集約して `NewsItem` 列を返す抽象 trait
#[async_trait::async_trait]
pub trait NewsAggregator: Send + Sync {
    async fn fetch_news(&self) -> Result<Vec<NewsItem>, DataProviderError>;
}
