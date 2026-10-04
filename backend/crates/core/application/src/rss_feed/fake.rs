use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::repository::{RssFeedRepository, RssFeedRepositoryError};
use super::types::{NewRssFeed, RssFeed, UpdateRssFeedPatch};

#[derive(Default)]
pub struct FakeRssFeedRepository {
    pub feeds: Mutex<Vec<RssFeed>>,
    pub transaction_ids: Mutex<Vec<Uuid>>,
}

impl FakeRssFeedRepository {
    pub fn new(feeds: Vec<RssFeed>) -> Self {
        Self {
            feeds: Mutex::new(feeds),
            transaction_ids: Mutex::new(Vec::new()),
        }
    }

    async fn record_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<(), RssFeedRepositoryError> {
        let transaction_id = transaction
            .downcast_ref::<FakeTransaction>()
            .map(|transaction| transaction.id)
            .ok_or(RssFeedRepositoryError::InvalidTransaction)?;
        self.transaction_ids.lock().await.push(transaction_id);
        Ok(())
    }
}

#[async_trait]
impl RssFeedRepository for FakeRssFeedRepository {
    async fn list(&self, enabled_only: bool) -> Result<Vec<RssFeed>, RssFeedRepositoryError> {
        let mut feeds: Vec<_> = self
            .feeds
            .lock()
            .await
            .iter()
            .filter(|feed| !enabled_only || feed.enabled)
            .cloned()
            .collect();
        feeds.sort_by(|left, right| left.display_name.cmp(&right.display_name));
        Ok(feeds)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<RssFeed>, RssFeedRepositoryError> {
        Ok(self
            .feeds
            .lock()
            .await
            .iter()
            .find(|feed| feed.id == id)
            .cloned())
    }

    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<RssFeed>, RssFeedRepositoryError> {
        self.record_transaction(transaction).await?;
        self.find_by_id(id).await
    }

    async fn create(
        &self,
        transaction: &UnitOfWorkTransaction,
        feed: NewRssFeed,
    ) -> Result<RssFeed, RssFeedRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut feeds = self.feeds.lock().await;
        if feeds.iter().any(|existing| existing.source == feed.source) {
            return Err(RssFeedRepositoryError::DuplicateSource(feed.source));
        }
        let now = Utc::now().fixed_offset();
        let feed = RssFeed {
            id: feed.id,
            source: feed.source,
            display_name: feed.display_name,
            url: feed.url,
            enabled: feed.enabled,
            content_source: feed.content_source,
            created_at: now,
            updated_at: now,
        };
        feeds.push(feed.clone());
        Ok(feed)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
        patch: UpdateRssFeedPatch,
        updated_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<RssFeed, RssFeedRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut feeds = self.feeds.lock().await;
        let Some(existing) = feeds.iter_mut().find(|existing| existing.id == id) else {
            return Err(RssFeedRepositoryError::Persistence(
                PersistenceError::RecordNotUpdated(format!("rss feed {id} was not updated")),
            ));
        };
        if let Some(display_name) = patch.display_name {
            existing.display_name = display_name;
        }
        if let Some(url) = patch.url {
            existing.url = url;
        }
        if let Some(enabled) = patch.enabled {
            existing.enabled = enabled;
        }
        if let Some(content_source) = patch.content_source {
            existing.content_source = content_source;
        }
        existing.updated_at = updated_at;
        Ok(existing.clone())
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, RssFeedRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut feeds = self.feeds.lock().await;
        let original_len = feeds.len();
        feeds.retain(|feed| feed.id != id);
        Ok(feeds.len() < original_len)
    }
}
