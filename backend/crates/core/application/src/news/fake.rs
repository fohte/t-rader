use std::collections::HashSet;

use async_trait::async_trait;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::news_aggregator::NewsItem;
use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::repository::{NewsItemRepository, NewsItemRepositoryError, NewsSearchCriteria};
use super::types::NewsArticle;
#[derive(Default)]
pub struct FakeNewsItemRepository {
    pub upserts: Mutex<Vec<Vec<NewsItem>>>,
    pub searches: Mutex<Vec<NewsSearchCriteria>>,
    pub transaction_ids: Mutex<Vec<Uuid>>,
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
    ) -> Result<usize, NewsItemRepositoryError> {
        self.record_transaction(transaction).await?;
        let urls: HashSet<_> = items.iter().map(|item| item.url.as_str()).collect();
        if urls.len() != items.len() {
            return Err(NewsItemRepositoryError::Persistence(
                crate::persistence::PersistenceError::Database(
                    "news item batch contains duplicate URLs".into(),
                ),
            ));
        }
        self.upserts.lock().await.push(items.to_vec());
        Ok(urls.len())
    }

    async fn search(
        &self,
        criteria: NewsSearchCriteria,
    ) -> Result<Vec<NewsArticle>, NewsItemRepositoryError> {
        self.searches.lock().await.push(criteria.clone());
        Ok(Vec::new())
    }
}
