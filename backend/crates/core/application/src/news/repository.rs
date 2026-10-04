use async_trait::async_trait;
use chrono::NaiveDate;
use thiserror::Error;

use crate::news_aggregator::NewsItem;
use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::NewsArticle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsSearchCriteria {
    pub keyword: Option<String>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub limit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertedNewsItem {
    pub id: uuid::Uuid,
    pub item: NewsItem,
    pub inserted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewsItemContentStatus {
    Pending,
    Fetched,
    Failed,
}

impl NewsItemContentStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Fetched => "fetched",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedNewsItemContent {
    pub news_item_id: uuid::Uuid,
    pub body: String,
}

/// 検索語をリテラルとして扱うため、LIKE の制御文字を除く。
pub fn sanitize_search_keyword(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '%' | '_' | '\\'))
        .collect()
}

#[derive(Debug, Error)]
pub enum NewsItemRepositoryError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait NewsItemRepository: Send + Sync {
    async fn upsert(
        &self,
        transaction: &UnitOfWorkTransaction,
        items: &[NewsItem],
    ) -> Result<Vec<UpsertedNewsItem>, NewsItemRepositoryError>;
    async fn create_pending_contents(
        &self,
        transaction: &UnitOfWorkTransaction,
        news_item_ids: &[uuid::Uuid],
    ) -> Result<(), NewsItemRepositoryError>;
    async fn upsert_fetched_contents(
        &self,
        transaction: &UnitOfWorkTransaction,
        contents: &[FetchedNewsItemContent],
    ) -> Result<(), NewsItemRepositoryError>;
    async fn search(
        &self,
        criteria: NewsSearchCriteria,
    ) -> Result<Vec<NewsArticle>, NewsItemRepositoryError>;
}

pub type SharedNewsItemRepository = std::sync::Arc<dyn NewsItemRepository + Send + Sync>;
