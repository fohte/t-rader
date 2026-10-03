use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::news_aggregator::NewsItem;
use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::repository::{
    FetchedNewsItemContent, NewsItemRepository, NewsItemRepositoryError, NewsItemUpsertResult,
    NewsSearchCriteria, UpsertedNewsItem, sanitize_search_keyword,
};
use super::types::NewsArticle;
#[derive(Default)]
pub struct FakeNewsItemRepository {
    pub upserts: Mutex<Vec<Vec<NewsItem>>>,
    pub searches: Mutex<Vec<NewsSearchCriteria>>,
    pub transaction_ids: Mutex<Vec<Uuid>>,
    pub articles: Mutex<Vec<NewsArticle>>,
    pub content_rows: Mutex<HashMap<Uuid, FakeNewsItemContent>>,
    pub upsert_error: Mutex<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeNewsItemContent {
    pub status: String,
    pub body: Option<String>,
    pub error: Option<String>,
}

impl FakeNewsItemRepository {
    pub fn new() -> Self {
        Self::default()
    }

    async fn record_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<(), NewsItemRepositoryError> {
        let transaction_id = transaction
            .downcast_ref::<FakeTransaction>()
            .map(|transaction| transaction.id)
            .ok_or(NewsItemRepositoryError::InvalidTransaction)?;
        self.transaction_ids.lock().await.push(transaction_id);
        Ok(())
    }
}

#[async_trait]
impl NewsItemRepository for FakeNewsItemRepository {
    async fn upsert(
        &self,
        transaction: &UnitOfWorkTransaction,
        items: &[NewsItem],
    ) -> Result<NewsItemUpsertResult, NewsItemRepositoryError> {
        self.record_transaction(transaction).await?;
        if let Some(message) = self.upsert_error.lock().await.clone() {
            return Err(NewsItemRepositoryError::Persistence(
                crate::persistence::PersistenceError::Database(message),
            ));
        }
        let urls: HashSet<_> = items.iter().map(|item| item.url.as_str()).collect();
        let mut articles = self.articles.lock().await;
        let mut seen = HashSet::new();
        let mut upserted_items = Vec::new();
        for item in items {
            if !seen.insert(item.url.as_str()) {
                continue;
            }
            if let Some(article) = articles.iter_mut().find(|article| article.url == item.url) {
                article.source = item.source.clone();
                article.title = item.title.clone();
                article.body_snippet = item.body_snippet.clone();
                article.published_at = item.published_at.fixed_offset();
                upserted_items.push(UpsertedNewsItem {
                    id: article.id,
                    item: item.clone(),
                    inserted: false,
                });
            } else {
                let id = Uuid::new_v4();
                articles.push(NewsArticle {
                    id,
                    source: item.source.clone(),
                    url: item.url.clone(),
                    title: item.title.clone(),
                    body_snippet: item.body_snippet.clone(),
                    published_at: item.published_at.fixed_offset(),
                });
                upserted_items.push(UpsertedNewsItem {
                    id,
                    item: item.clone(),
                    inserted: true,
                });
            }
        }
        drop(articles);
        self.upserts.lock().await.push(items.to_vec());
        Ok(NewsItemUpsertResult {
            fetched: urls.len(),
            items: upserted_items,
        })
    }

    async fn create_pending_contents(
        &self,
        transaction: &UnitOfWorkTransaction,
        news_item_ids: &[Uuid],
    ) -> Result<(), NewsItemRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut content_rows = self.content_rows.lock().await;
        for id in news_item_ids {
            content_rows
                .entry(*id)
                .or_insert_with(|| FakeNewsItemContent {
                    status: "pending".into(),
                    body: None,
                    error: None,
                });
        }
        Ok(())
    }

    async fn upsert_fetched_contents(
        &self,
        transaction: &UnitOfWorkTransaction,
        contents: &[FetchedNewsItemContent],
    ) -> Result<(), NewsItemRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut content_rows = self.content_rows.lock().await;
        for content in contents {
            content_rows.insert(
                content.news_item_id,
                FakeNewsItemContent {
                    status: "fetched".into(),
                    body: Some(content.body.clone()),
                    error: None,
                },
            );
        }
        Ok(())
    }

    async fn search(
        &self,
        criteria: NewsSearchCriteria,
    ) -> Result<Vec<NewsArticle>, NewsItemRepositoryError> {
        self.searches.lock().await.push(criteria.clone());
        let keyword = criteria
            .keyword
            .as_deref()
            .map(sanitize_search_keyword)
            .map(|value| value.to_lowercase());
        let mut articles: Vec<_> = self
            .articles
            .lock()
            .await
            .iter()
            .filter(|article| {
                let published_on = article.published_at.date_naive();
                criteria.from.is_none_or(|from| published_on >= from)
                    && criteria.to.is_none_or(|to| published_on <= to)
                    && keyword.as_ref().is_none_or(|keyword| {
                        article.title.to_lowercase().contains(keyword)
                            || article
                                .body_snippet
                                .as_deref()
                                .is_some_and(|snippet| snippet.to_lowercase().contains(keyword))
                    })
            })
            .cloned()
            .collect();
        articles.sort_by_key(|article| std::cmp::Reverse(article.published_at));
        Ok(articles
            .into_iter()
            .take(usize::try_from(criteria.limit).unwrap_or(usize::MAX))
            .collect())
    }
}
